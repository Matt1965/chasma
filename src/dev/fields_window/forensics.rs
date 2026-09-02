//! Runtime forensics for Fields window ECS hierarchy (env-gated).
//!
//! Modes (`CHASMA_FIELDS_FORENSICS`):
//! - `launcher` — simulate Advanced -> Fields click path with per-sync checkpoints
//! - `click`    — trace real Fields launcher click only (no auto-exit)
//! - `1`/`true` — legacy baseline: direct `registry.show(Fields)` on frame 1
//!
//! Investigation only — not part of normal gameplay.

use bevy::prelude::*;
use bevy::ui::UiSystems;

use crate::dev::dev_mode::DevModeState;
use crate::dev::terrain_field::{DevTerrainFieldButton, DevTerrainFieldSection};
use crate::dev::widgets::{
    DevCollapsibleBody, DevCollapsibleSection, DevCollapsibleSectionId, DevCollapsibleState,
};
use crate::dev::window::{
    DevWindowBody, DevWindowCollapseButton, DevWindowId, DevWindowRegistry, DevWindowRoot,
    DevWorkspaceLauncherButton,
};

use super::panel::DevFieldsWindowUi;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Resource)]
pub enum FieldsForensicsMode {
    /// Direct `registry.show(Fields)` baseline (known-good from prior run).
    Baseline,
    /// Scripted Advanced launcher expand -> Fields toggle.
    LauncherPath,
    /// Wait for real Fields launcher button press.
    ClickTrace,
}

pub fn fields_forensics_mode() -> Option<FieldsForensicsMode> {
    match std::env::var("CHASMA_FIELDS_FORENSICS")
        .ok()
        .map(|v| v.to_ascii_lowercase())
        .as_deref()
    {
        Some("launcher") => Some(FieldsForensicsMode::LauncherPath),
        Some("click") => Some(FieldsForensicsMode::ClickTrace),
        Some("1") | Some("true") => Some(FieldsForensicsMode::Baseline),
        _ => None,
    }
}

pub fn fields_forensics_enabled() -> bool {
    fields_forensics_mode().is_some()
}

#[derive(Resource, Default)]
struct FieldsForensicsState {
    frames: u32,
    baseline_logged: bool,
}

#[derive(Resource, Default)]
pub(crate) struct FieldsLauncherTrace {
    /// Capture checkpoints for the current open attempt.
    pub active: bool,
    pub click_frame: u32,
    pub checkpoints_done: u8,
    pub awaiting_post_layout: bool,
    pub scripted_step: u8,
    pub scripted_done: bool,
    pub comparison_logged: bool,
}

const CP_BEFORE_CLICK: u8 = 1;
const CP_AFTER_CLICK: u8 = 2;
const CP_AFTER_PRESENTATION: u8 = 3;
const CP_AFTER_FIELDS_VISIBILITY: u8 = 4;
const CP_AFTER_COLLAPSIBLE: u8 = 5;
const CP_AFTER_LAYOUT: u8 = 6;

fn format_computed(world: &mut World, entity: Entity) -> String {
    match world.get::<ComputedNode>(entity) {
        Some(node) => {
            let size = node.size();
            format!("computed={:.1}x{:.1}", size.x, size.y)
        }
        None => "computed=<missing>".into(),
    }
}

fn format_node(world: &mut World, entity: Entity) -> String {
    let Some(node) = world.get::<Node>(entity) else {
        return "Node=<missing>".into();
    };
    format!(
        "display={:?} width={:?} height={:?} min_h={:?} max_h={:?}",
        node.display, node.width, node.height, node.min_height, node.max_height
    )
}

fn format_visibility(world: &mut World, entity: Entity) -> String {
    format!(
        "{:?}",
        world
            .get::<Visibility>(entity)
            .copied()
            .unwrap_or(Visibility::default())
    )
}

fn find_fields_root(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<DevWindowRoot>>()
        .iter(world)
        .find(|entity| {
            world
                .get::<DevWindowRoot>(*entity)
                .is_some_and(|root| root.id == DevWindowId::Fields)
        })
}

fn find_fields_body(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<DevWindowBody>>()
        .iter(world)
        .find(|entity| {
            world
                .get::<DevWindowBody>(*entity)
                .is_some_and(|body| body.id == DevWindowId::Fields)
        })
}

fn find_fields_panel(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<DevFieldsWindowUi>>()
        .iter(world)
        .next()
}

fn find_fields_section(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<DevCollapsibleSection>>()
        .iter(world)
        .find(|entity| {
            world
                .get::<DevCollapsibleSection>(*entity)
                .is_some_and(|section| section.id == DevCollapsibleSectionId::FieldsBuild)
        })
}

fn find_fields_collapsible_body(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<DevCollapsibleBody>>()
        .iter(world)
        .find(|entity| {
            world
                .get::<DevCollapsibleBody>(*entity)
                .is_some_and(|body| body.id == DevCollapsibleSectionId::FieldsBuild)
        })
}

fn find_section_header_row(world: &mut World, section: Entity) -> Option<Entity> {
    let children = world.get::<Children>(section)?;
    children.iter().find(|child| {
        world.get::<Node>(*child).is_some_and(|node| {
            matches!(node.flex_direction, FlexDirection::Row) && node.display != Display::None
        }) || world
            .get::<Children>(*child)
            .is_some_and(|grandchildren| grandchildren.len() >= 2)
    })
}

fn find_build_field_label(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<Text>>()
        .iter(world)
        .find(|entity| {
            world
                .get::<Text>(*entity)
                .is_some_and(|text| text.as_str().contains("Build field"))
        })
}

fn log_entity(world: &mut World, entity: Entity, role: &str) {
    eprintln!("  [{role}] entity={entity:?}");
    eprintln!("    Visibility={}", format_visibility(world, entity));
    eprintln!("    {}", format_node(world, entity));
    eprintln!("    {}", format_computed(world, entity));
    if let Some(text) = world.get::<Text>(entity) {
        eprintln!("    Text={:?}", text.as_str());
    }
    if let Some(color) = world.get::<TextColor>(entity) {
        eprintln!("    TextColor={:?}", color.0);
    }
}

pub(crate) fn log_fields_launcher_snapshot(world: &mut World, checkpoint: u8, label: &str) {
    eprintln!("=== FIELDS LAUNCHER TRACE checkpoint {checkpoint}: {label} ===");

    if let Some(registry) = world.get_resource::<DevWindowRegistry>() {
        if let Some(session) = registry.session(DevWindowId::Fields) {
            eprintln!(
                "WINDOW SESSION: visible={} collapsed={} computed_size={:.1}x{:.1}",
                session.visible,
                session.collapsed,
                session.computed_size.x,
                session.computed_size.y
            );
        } else {
            eprintln!("WINDOW SESSION: <no Fields session>");
        }
        eprintln!(
            "LAUNCHER STATE: advanced_expanded={} launcher_expanded={}",
            registry.advanced_launcher_expanded, registry.launcher_expanded
        );
    }
    if let Some(dev) = world.get_resource::<DevModeState>() {
        eprintln!("DevModeState.enabled={}", dev.enabled);
    }
    if let Some(collapsible) = world.get_resource::<DevCollapsibleState>() {
        let expanded = collapsible.is_expanded(DevCollapsibleSectionId::FieldsBuild);
        eprintln!("DevCollapsibleState FieldsBuild expanded={expanded}");
    }

    if let Some(root) = find_fields_root(world) {
        log_entity(world, root, "VISIBLE ROOT DevWindowRoot(Fields)");
        if let Some(registry) = world.get_resource::<DevWindowRegistry>() {
            if let Some(session) = registry.session(DevWindowId::Fields) {
                eprintln!(
                    "  registry.computed_size for root={:.1}x{:.1}",
                    session.computed_size.x, session.computed_size.y
                );
            }
        }
    } else {
        eprintln!("  VISIBLE ROOT: <missing>");
    }

    for (button_entity, button) in world
        .query::<(Entity, &DevWindowCollapseButton)>()
        .iter(world)
        .filter(|(_, btn)| btn.id == DevWindowId::Fields)
    {
        if let Some(children) = world.get::<Children>(button_entity) {
            for child in children.iter() {
                if let Some(text) = world.get::<Text>(child) {
                    eprintln!(
                        "  COLLAPSE CHROME LABEL entity={child:?} text={:?}",
                        text.as_str()
                    );
                }
            }
        }
    }

    if let Some(body) = find_fields_body(world) {
        log_entity(world, body, "BODY DevWindowBody(Fields)");
    } else {
        eprintln!("  BODY: <missing>");
    }

    if let Some(panel) = find_fields_panel(world) {
        log_entity(world, panel, "PANEL DevFieldsWindowUi");
    } else {
        eprintln!("  PANEL: <missing>");
    }

    if let Some(section) = find_fields_section(world) {
        log_entity(world, section, "COLLAPSIBLE SECTION FieldsBuild");
        if let Some(header) = find_section_header_row(world, section) {
            log_entity(world, header, "HEADER ROW");
        } else {
            eprintln!("  HEADER ROW: <missing>");
        }
    } else {
        eprintln!("  COLLAPSIBLE SECTION: <missing>");
    }

    if let Some(body) = find_fields_collapsible_body(world) {
        log_entity(world, body, "COLLAPSIBLE BODY FieldsBuild");
    } else {
        eprintln!("  COLLAPSIBLE BODY: <missing>");
    }

    if let Some(section) = world
        .query_filtered::<Entity, With<DevTerrainFieldSection>>()
        .iter(world)
        .next()
    {
        log_entity(world, section, "TERRAIN SECTION");
    } else {
        eprintln!("  TERRAIN SECTION: <missing>");
    }

    if let Some(label) = find_build_field_label(world) {
        log_entity(world, label, "DESCENDANT \"Build field\"");
        if let Some(button) = world.get::<DevTerrainFieldButton>(label) {
            eprintln!("    DevTerrainFieldButton action={:?}", button.action);
        }
        if world.get::<Button>(label).is_some() {
            eprintln!("    has Button component");
        } else if let Some(parent) = world.get::<ChildOf>(label).map(|c| c.parent()) {
            eprintln!(
                "    Button on parent={parent:?}: {}",
                world.get::<Button>(parent).is_some()
            );
        }
    } else {
        eprintln!("  DESCENDANT \"Build field\": <missing>");
    }

    eprintln!("=== END checkpoint {checkpoint} ===");
}

fn log_checkpoint(world: &mut World, checkpoint: u8, label: &str) {
    if world.resource::<FieldsLauncherTrace>().checkpoints_done >= checkpoint {
        return;
    }
    log_fields_launcher_snapshot(world, checkpoint, label);
    world.resource_mut::<FieldsLauncherTrace>().checkpoints_done = checkpoint;
}

/// Scripted manual path: dev on -> Advanced expand -> Fields toggle.
pub fn fields_launcher_trace_scripted(world: &mut World) {
    let mode = match world.get_resource::<FieldsForensicsMode>() {
        Some(mode) => *mode,
        None => return,
    };
    if mode != FieldsForensicsMode::LauncherPath {
        return;
    }
    if world.resource::<FieldsLauncherTrace>().scripted_done {
        return;
    }

    let frame = {
        let mut frames = world.resource_mut::<FieldsForensicsState>();
        frames.frames += 1;
        frames.frames
    };

    let step = world.resource::<FieldsLauncherTrace>().scripted_step;
    match step {
        0 if frame == 1 => {
            world.resource_mut::<DevModeState>().enabled = true;
            eprintln!("FIELDS LAUNCHER TRACE: frame 1 — dev mode enabled (F12 equivalent)");
            world.resource_mut::<FieldsLauncherTrace>().scripted_step = 1;
        }
        1 if frame == 2 => {
            world
                .resource_mut::<DevWindowRegistry>()
                .advanced_launcher_expanded = true;
            eprintln!("FIELDS LAUNCHER TRACE: frame 2 — Advanced launcher expanded");
            world.resource_mut::<FieldsLauncherTrace>().scripted_step = 2;
        }
        2 if frame >= 3 && !world.resource::<FieldsLauncherTrace>().active => {
            world.resource_mut::<FieldsLauncherTrace>().active = true;
            world.resource_mut::<FieldsLauncherTrace>().click_frame = frame;
            log_checkpoint(world, CP_BEFORE_CLICK, "before Fields launcher toggle");
            let was_visible = world
                .resource::<DevWindowRegistry>()
                .is_visible(DevWindowId::Fields);
            world
                .resource_mut::<DevWindowRegistry>()
                .toggle(DevWindowId::Fields);
            let now_visible = world
                .resource::<DevWindowRegistry>()
                .is_visible(DevWindowId::Fields);
            eprintln!(
                "FIELDS LAUNCHER TRACE: toggled Fields (was_visible={was_visible} now_visible={now_visible})"
            );
            log_checkpoint(world, CP_AFTER_CLICK, "after Fields launcher toggle");
            world.resource_mut::<FieldsLauncherTrace>().scripted_step = 3;
        }
        _ => {}
    }
}

/// Detect real Fields launcher click — checkpoint 1 before handler runs.
pub fn fields_launcher_trace_before_click(world: &mut World) {
    let mode = match world.get_resource::<FieldsForensicsMode>() {
        Some(mode) => *mode,
        None => return,
    };
    if mode != FieldsForensicsMode::ClickTrace {
        return;
    }
    if world.resource::<FieldsLauncherTrace>().active
        || world.resource::<FieldsLauncherTrace>().scripted_done
    {
        return;
    }

    let mut fields_pressed = false;
    for (interaction, button) in world
        .query::<(&Interaction, &DevWorkspaceLauncherButton)>()
        .iter(world)
    {
        if *interaction == Interaction::Pressed && button.window == DevWindowId::Fields {
            fields_pressed = true;
            break;
        }
    }
    if !fields_pressed {
        return;
    }

    let frame = world.resource::<FieldsForensicsState>().frames;
    {
        let mut trace = world.resource_mut::<FieldsLauncherTrace>();
        trace.active = true;
        trace.click_frame = frame;
    }
    log_checkpoint(
        world,
        CP_BEFORE_CLICK,
        "before handle_dev_window_pointer Fields click",
    );
}

/// Checkpoint 2 — immediately after click handler.
pub fn fields_launcher_trace_after_click(world: &mut World) {
    let mode = match world.get_resource::<FieldsForensicsMode>() {
        Some(mode) => *mode,
        None => return,
    };
    if mode != FieldsForensicsMode::ClickTrace {
        return;
    }
    if !world.resource::<FieldsLauncherTrace>().active
        || world.resource::<FieldsLauncherTrace>().checkpoints_done >= CP_AFTER_CLICK
    {
        return;
    }
    log_checkpoint(
        world,
        CP_AFTER_CLICK,
        "after handle_dev_window_pointer Fields click",
    );
}

pub fn fields_launcher_trace_after_presentation(world: &mut World) {
    if !world.resource::<FieldsLauncherTrace>().active {
        return;
    }
    log_checkpoint(
        world,
        CP_AFTER_PRESENTATION,
        "after sync_dev_window_presentation",
    );
}

pub fn fields_launcher_trace_after_fields_visibility(world: &mut World) {
    if !world.resource::<FieldsLauncherTrace>().active {
        return;
    }
    log_checkpoint(
        world,
        CP_AFTER_FIELDS_VISIBILITY,
        "after sync_dev_fields_panel_visibility",
    );
}

pub fn fields_launcher_trace_after_collapsible(world: &mut World) {
    if !world.resource::<FieldsLauncherTrace>().active {
        return;
    }
    log_checkpoint(
        world,
        CP_AFTER_COLLAPSIBLE,
        "after sync_collapsible_sections",
    );
    world
        .resource_mut::<FieldsLauncherTrace>()
        .awaiting_post_layout = true;
}

pub fn fields_launcher_trace_post_layout(world: &mut World) {
    if !world.resource::<FieldsLauncherTrace>().awaiting_post_layout
        || !world.resource::<FieldsLauncherTrace>().active
    {
        return;
    }
    log_checkpoint(world, CP_AFTER_LAYOUT, "PostUpdate after UiSystems::Layout");
    {
        let mut trace = world.resource_mut::<FieldsLauncherTrace>();
        trace.awaiting_post_layout = false;
        trace.scripted_done = true;
        trace.active = false;
    }

    if *world.resource::<FieldsForensicsMode>() == FieldsForensicsMode::LauncherPath {
        eprintln!("FIELDS LAUNCHER TRACE: launcher path complete — logging baseline comparison");
        log_baseline_comparison(world);
    }
}

fn log_baseline_comparison(world: &mut World) {
    let mut trace = world.resource_mut::<FieldsLauncherTrace>();
    if trace.comparison_logged {
        return;
    }
    trace.comparison_logged = true;
    drop(trace);

    {
        let mut registry = world.resource_mut::<DevWindowRegistry>();
        registry.hide(DevWindowId::Fields);
    }
    eprintln!("=== BASELINE COMPARISON: direct registry.show(Fields) ===");
    {
        let mut registry = world.resource_mut::<DevWindowRegistry>();
        registry.show(DevWindowId::Fields);
    }
    log_fields_launcher_snapshot(world, 99, "known-good direct show (no launcher path)");

    eprintln!("FIELDS LAUNCHER TRACE: exiting (CHASMA_FIELDS_FORENSICS=launcher)");
    world.write_message(AppExit::Success);
}

pub fn fields_forensics_post_startup(world: &mut World) {
    if let Some(mode) = fields_forensics_mode() {
        world.init_resource::<FieldsForensicsState>();
        world.init_resource::<FieldsLauncherTrace>();
        world.insert_resource(mode);
        if mode == FieldsForensicsMode::Baseline {
            log_fields_launcher_snapshot(world, 0, "PostStartup before any open");
        }
    }
}

pub fn fields_forensics_tick_frames(world: &mut World) {
    let Some(mode) = fields_forensics_mode() else {
        return;
    };
    if mode == FieldsForensicsMode::Baseline {
        return;
    }
    let mut frames = world.resource_mut::<FieldsForensicsState>();
    frames.frames += 1;
}

pub fn fields_forensics_update(world: &mut World) {
    let Some(mode) = fields_forensics_mode() else {
        return;
    };
    if mode != FieldsForensicsMode::Baseline {
        return;
    }

    let frames = {
        let mut state = world.resource_mut::<FieldsForensicsState>();
        state.frames += 1;
        state.frames
    };

    if frames == 1 {
        world.resource_mut::<DevModeState>().enabled = true;
        world
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Fields);
        eprintln!("FIELDS FORENSICS: frame 1 — baseline direct show(Fields)");
    }

    if frames == 3 {
        log_fields_launcher_snapshot(world, 99, "baseline frame 3 after sync (prior known-good)");
    }

    if frames >= 4 {
        eprintln!("FIELDS FORENSICS: exiting (CHASMA_FIELDS_FORENSICS=baseline)");
        world.write_message(AppExit::Success);
    }
}
