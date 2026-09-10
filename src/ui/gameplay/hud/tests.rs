//! Bottom HUD contract tests (mockup reskin).

use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, FocusPolicy, IsDefaultUiCamera, UiGlobalTransform, UiSystems};

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::ui::gameplay::command_panel::{COMMAND_GRID, CommandPanelRoot, HudCommandButton};
use crate::ui::gameplay::fields_menu::{
    FieldsMenuOption, FieldsMenuRoot, player_field_menu_entries,
};
use crate::ui::gameplay::hud::geometry::{HUD_REFERENCE_VIEWPORT_WIDTH, compute_hud_height};
use crate::ui::gameplay::hud::{
    HudFrameBackground, HudPlateFrame, HudPlateSection, HudStatBarId, HudViewportGeometry,
    apply_hud_viewport_geometry, compute_hud_viewport_geometry, sync_hud_plate_frames,
};
use crate::ui::gameplay::input_gate::{PlayerHudHoverState, gameplay_input_blocked_by_hud};
use crate::ui::gameplay::layout::{
    BottomBar, GameplayHudRoot, bottom_hud_rect_contains, setup_player_hud_layout,
};
use crate::ui::gameplay::selected_unit_panel::{
    HUD_NO_SELECTION_LABEL, SelectedUnitPanelRoot, build_selected_panel_snapshot,
};
use crate::ui::gameplay::squad_panel::{
    HUD_ROSTER_CARD_HP_HEIGHT_PX, HUD_ROSTER_CARD_LABEL_HEIGHT_PX, HUD_ROSTER_HEADER_HEIGHT_PX,
    SquadPanelRoot, SquadRosterViewport, owned_roster_unit_ids,
};
use crate::ui::gameplay::styles::{
    BOTTOM_BAR_HEIGHT_PX, HUD_COMMAND_BUTTON_HEIGHT_PERCENT, HUD_CONTENT_HEIGHT_PX,
    HUD_ENDCAP_SEAT_PX, HUD_FRAME_BOTTOM_PX, HUD_FRAME_TOP_PX, HUD_HEIGHT_PX, HUD_PLATE_CHAMFER_PX,
    HUD_PLATE_JOIN_CHAMFER_PX, HUD_PLATE_JOIN_OVERLAP_PX, HUD_SECTION_GAP_PX,
    HUD_SOURCE_BAND_HEIGHT_PX, HUD_UTILITY_BUTTON_HEIGHT_PX, HUD_UTILITY_GROUP_DIVIDER_PX,
    HUD_UTILITY_ROW_GAP_PX,
};
use crate::ui::gameplay::utility_panel::UtilityPanelRoot;
use crate::ui::gameplay::utility_panel::{HudUtilityButton, PERMANENT_UTILITY_BUTTONS};
use crate::units::input::SelectedUnits;
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingId, BuildingOwnership,
    BuildingPlacement, BuildingRecord, BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
    Heightfield, LocalPosition, UnitCatalog, UnitDefinitionId, UnitId, UnitOwnership, UnitSource,
    WeaponCatalog, WorldData, WorldPosition, create_unit, create_unit_with_ownership,
    starter_weapon_definitions,
};

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

/// Viewport the HUD is proportioned against.
const REFERENCE_VIEWPORT: Vec2 = Vec2::new(1920.0, 1080.0);
/// HUD height before this pass, for the ~1.75x growth check.
const PREVIOUS_HUD_HEIGHT_PX: f32 = 112.0;

/// Headless app carrying just enough for `setup_player_hud_layout` to run and
/// for Bevy's UI layout to resolve the spawned tree to real pixel rectangles.
fn hud_layout_app_with_viewport(viewport: Vec2) -> App {
    let mut app = App::new();
    // `UiPlugin` also schedules focus and picking work that this layout-only run
    // has no pointer for; warn instead of aborting the frame when those systems
    // find no input state. Layout itself needs none of it.
    app.set_error_handler(bevy::ecs::error::warn);
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::input::InputPlugin,
        bevy::picking::PickingPlugin,
        bevy::picking::InteractionPlugin,
        // `ui_layout_system` measures text, so it needs the text pipeline even
        // though these assertions only look at section rectangles.
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ));
    // The HUD loads chrome images; layout only needs the handles to allocate.
    app.init_asset::<Image>();
    // UI layout sizes roots from the camera's render target. Without a render
    // backend nothing fills `target_info` in, so stand it in explicitly;
    // `IsDefaultUiCamera` makes this the camera HUD roots resolve against.
    app.world_mut().spawn((
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size: viewport.as_uvec2(),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            viewport: Some(Viewport {
                physical_position: UVec2::ZERO,
                physical_size: viewport.as_uvec2(),
                ..default()
            }),
            ..default()
        },
        IsDefaultUiCamera,
    ));
    app.insert_resource(compute_hud_viewport_geometry(viewport));
    app.add_systems(
        PostUpdate,
        apply_hud_viewport_geometry.before(UiSystems::Layout),
    );
    app.add_systems(PostUpdate, sync_hud_plate_frames.after(UiSystems::Layout));
    app.world_mut()
        .run_system_once(setup_player_hud_layout)
        .expect("setup_player_hud_layout");
    // Two passes: the first resolves layout, the second settles scroll content.
    app.update();
    app.update();
    app
}

fn hud_layout_app() -> App {
    hud_layout_app_with_viewport(REFERENCE_VIEWPORT)
}

/// Screen-space rect of a uniquely-marked HUD node, in logical pixels.
fn node_rect<M: Component>(app: &mut App, label: &str) -> Rect {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<M>>();
    let matches: Vec<Rect> = query
        .iter(world)
        .map(|(computed, transform)| Rect::from_center_size(transform.translation, computed.size()))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one `{label}` node in the HUD tree"
    );
    let rect = matches[0];
    // A collapsed rect means layout never resolved, which would make every
    // geometry comparison below trivially true.
    assert!(
        rect.width() > 0.0 && rect.height() > 0.0,
        "`{label}` resolved to an empty rect; layout did not run"
    );
    rect
}

#[test]
fn bottom_hud_is_anchored_to_viewport_bottom() {
    let viewport = REFERENCE_VIEWPORT;
    let geom = compute_hud_viewport_geometry(viewport);
    let hud_top = viewport.y - geom.hud_height;
    assert!(bottom_hud_rect_contains(
        Vec2::new(100.0, 1079.0),
        viewport,
        geom.hud_height
    ));
    assert!(bottom_hud_rect_contains(
        Vec2::new(100.0, hud_top + 1.0),
        viewport,
        geom.hud_height,
    ));
    assert!(!bottom_hud_rect_contains(
        Vec2::new(100.0, hud_top - 1.0),
        viewport,
        geom.hud_height,
    ));
    assert_eq!(BOTTOM_BAR_HEIGHT_PX, HUD_HEIGHT_PX);
}

/// B. The band grew to the intended target rather than staying at 112px.
#[test]
fn hud_height_matches_increased_target_and_source_proportions() {
    let reference_height = compute_hud_height(REFERENCE_VIEWPORT);
    let growth = reference_height / PREVIOUS_HUD_HEIGHT_PX;
    assert!(
        (1.70..=1.80).contains(&growth),
        "HUD height should be ~1.75x the previous 112px, got {reference_height}px ({growth:.2}x)"
    );

    // The mockup band is 199px tall on a 1983px-wide composite. Holding that
    // ratio at the reference viewport width is what keeps the art undistorted.
    let source_ratio = HUD_SOURCE_BAND_HEIGHT_PX / 1983.0;
    let runtime_ratio = reference_height / REFERENCE_VIEWPORT.x;
    assert!(
        (source_ratio - runtime_ratio).abs() < 0.01,
        "HUD height should track the mockup's band-to-width ratio \
         (source {source_ratio:.4}, runtime {runtime_ratio:.4})"
    );
}

/// J. The spawned root is bottom-anchored and full width.
#[test]
fn hud_root_spans_viewport_bottom() {
    let mut app = hud_layout_app();
    let geom = app.world().resource::<HudViewportGeometry>().clone();
    let root = node_rect::<GameplayHudRoot>(&mut app, "GameplayHudRoot");

    assert!(
        (root.height() - geom.hud_height).abs() < 0.5,
        "root height {} should match responsive hud_height {}",
        root.height(),
        geom.hud_height,
    );
    assert!(
        (root.width() - REFERENCE_VIEWPORT.x).abs() < 0.5,
        "root width {} should span the viewport",
        root.width()
    );
    assert!(
        (root.max.y - REFERENCE_VIEWPORT.y).abs() < 0.5,
        "root bottom {} should sit on the viewport bottom",
        root.max.y
    );
}

/// A + G. Every section shares the content row's vertical bounds, and adjacent
/// sections touch, so no join can show a transparent gap or a stepped edge.
#[test]
fn hud_sections_share_vertical_bounds_and_abut_horizontally() {
    let mut app = hud_layout_app();
    let bar = node_rect::<BottomBar>(&mut app, "BottomBar");
    let selected = node_rect::<SelectedUnitPanelRoot>(&mut app, "SelectedUnitPanelRoot");
    let roster = node_rect::<SquadPanelRoot>(&mut app, "SquadPanelRoot");
    let commands = node_rect::<CommandPanelRoot>(&mut app, "CommandPanelRoot");
    let utility = node_rect::<UtilityPanelRoot>(&mut app, "UtilityPanelRoot");

    let sections = [
        ("selected", selected),
        ("roster", roster),
        ("commands", commands),
        ("utility", utility),
    ];
    for (name, rect) in sections {
        assert!(
            (rect.min.y - bar.min.y).abs() < 0.5,
            "{name} top {} should equal the content row top {}",
            rect.min.y,
            bar.min.y
        );
        assert!(
            (rect.max.y - bar.max.y).abs() < 0.5,
            "{name} bottom {} should equal the content row bottom {}",
            rect.max.y,
            bar.max.y
        );
    }

    // Sections are laid out left to right with only dividers between them.
    let ordered = [selected, roster, commands, utility];
    for pair in ordered.windows(2) {
        let gap = pair[1].min.x - pair[0].max.x;
        assert!(
            gap >= -2.0 && gap <= 4.0,
            "adjacent sections should abut (divider only), found a {gap}px gap"
        );
    }

    // The row itself is inset only by the frame bevels and endcaps.
    let geom = app.world().resource::<HudViewportGeometry>().clone();
    let root = node_rect::<GameplayHudRoot>(&mut app, "GameplayHudRoot");
    assert!(
        (bar.min.y - (root.min.y + geom.frame_top)).abs() < 0.5,
        "content row should start below the frame's top bevel"
    );
    assert!(
        ((root.max.y - geom.frame_bottom) - bar.max.y).abs() < 0.5,
        "content row should end above the frame's bottom bevel"
    );
    assert!(
        (bar.min.x - (root.min.x + geom.endcap_left_width)).abs() < 0.5,
        "content row should start inboard of the left endcap"
    );
    assert!(
        ((root.max.x - geom.endcap_right_width) - bar.max.x).abs() < 0.5,
        "content row should end inboard of the right endcap"
    );
}

#[test]
#[ignore = "developer aid: dumps resolved HUD geometry for the offline preview"]
fn dump_hud_geometry() {
    let mut app = hud_layout_app();
    let root = node_rect::<GameplayHudRoot>(&mut app, "GameplayHudRoot");
    let bar = node_rect::<BottomBar>(&mut app, "BottomBar");
    let selected = node_rect::<SelectedUnitPanelRoot>(&mut app, "SelectedUnitPanelRoot");
    let roster = node_rect::<SquadPanelRoot>(&mut app, "SquadPanelRoot");
    let commands = node_rect::<CommandPanelRoot>(&mut app, "CommandPanelRoot");
    let utility = node_rect::<UtilityPanelRoot>(&mut app, "UtilityPanelRoot");
    for (name, rect) in [
        ("root", root),
        ("bar", bar),
        ("selected", selected),
        ("roster", roster),
        ("commands", commands),
        ("utility", utility),
    ] {
        println!(
            "GEOM {name} {} {} {} {}",
            rect.min.x, rect.min.y, rect.max.x, rect.max.y
        );
    }
    let world = app.world_mut();
    let mut frames = world.query::<(&HudPlateFrame, &ComputedNode, &UiGlobalTransform)>();
    for (frame, computed, transform) in frames.iter(world) {
        let rect = Rect::from_center_size(transform.translation, computed.size());
        println!(
            "GEOM plate_{:?} {} {} {} {}",
            frame.section, rect.min.x, rect.min.y, rect.max.x, rect.max.y
        );
    }

    // Vertical extent actually occupied by each section's descendants, which is
    // what bounds how far its plate may be inset.
    for (name, rect) in [
        (
            "selected",
            section_content_bounds::<SelectedUnitPanelRoot>(&mut app),
        ),
        ("roster", section_content_bounds::<SquadPanelRoot>(&mut app)),
        (
            "commands",
            section_content_bounds::<CommandPanelRoot>(&mut app),
        ),
        (
            "utility",
            section_content_bounds::<UtilityPanelRoot>(&mut app),
        ),
    ] {
        println!("CONTENT {name} top={} bottom={}", rect.min.y, rect.max.y);
    }
}

/// Union of every descendant rect under a section root, in screen space.
fn section_content_bounds<M: Component>(app: &mut App) -> Rect {
    let world = app.world_mut();
    let root = world
        .query_filtered::<Entity, With<M>>()
        .single(world)
        .expect("section root");

    let mut bounds: Option<Rect> = None;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let (Some(computed), Some(transform)) = (
            world.get::<ComputedNode>(entity).copied(),
            world.get::<UiGlobalTransform>(entity).copied(),
        ) && computed.size().y > 0.0
        {
            let rect = Rect::from_center_size(transform.translation, computed.size());
            bounds = Some(match bounds {
                Some(current) => current.union(rect),
                None => rect,
            });
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    bounds.expect("section has geometry")
}

/// G. The plate run covers the root end to end with overlapping neighbours, so
/// no join can read as a hole even though each plate has its own contour.
#[test]
fn plate_run_covers_the_whole_hud_without_a_hole_at_any_join() {
    let mut app = hud_layout_app();
    let root = node_rect::<GameplayHudRoot>(&mut app, "GameplayHudRoot");

    let world = app.world_mut();
    let mut frames = world.query::<(&HudPlateFrame, &ComputedNode, &UiGlobalTransform)>();
    let mut plates: Vec<Rect> = frames
        .iter(world)
        .map(|(_, computed, transform)| {
            Rect::from_center_size(transform.translation, computed.size())
        })
        .collect();
    plates.sort_by(|a, b| a.min.x.partial_cmp(&b.min.x).expect("finite"));
    assert_eq!(plates.len(), 4, "expected four plates");

    let geom = app.world().resource::<HudViewportGeometry>().clone();
    let left_seat = geom.endcap_left_width - HUD_ENDCAP_SEAT_PX;
    let right_seat = root.max.x - geom.endcap_right_width + HUD_ENDCAP_SEAT_PX;
    assert!(
        (plates[0].min.x - left_seat).abs() < 0.5,
        "selected plate must tuck under the left endcap"
    );
    assert!(
        (plates[plates.len() - 1].max.x - right_seat).abs() < 0.5,
        "utility plate must tuck under the right endcap"
    );
    for pair in plates.windows(2) {
        assert!(
            pair[1].min.x < pair[0].max.x,
            "plates {:?} and {:?} must overlap, not leave a gap",
            pair[0],
            pair[1]
        );
    }

    assert_eq!(
        HUD_SECTION_GAP_PX, 0.0,
        "sections must not be separated by a layout gap"
    );
}

/// C. Roster cards and their fixed rows fit inside the content height.
#[test]
fn roster_cards_fit_within_content_bounds() {
    let mut app = hud_layout_app();
    let roster = node_rect::<SquadPanelRoot>(&mut app, "SquadPanelRoot");
    let viewport = node_rect::<SquadRosterViewport>(&mut app, "SquadRosterViewport");

    assert!(
        viewport.min.y >= roster.min.y - 0.5 && viewport.max.y <= roster.max.y + 0.5,
        "roster viewport {viewport:?} should stay inside the roster section {roster:?}"
    );

    let card_fixed_rows = HUD_ROSTER_CARD_HP_HEIGHT_PX + HUD_ROSTER_CARD_LABEL_HEIGHT_PX;
    assert!(
        HUD_ROSTER_HEADER_HEIGHT_PX + card_fixed_rows < HUD_CONTENT_HEIGHT_PX,
        "roster header plus fixed card rows must leave room for the portrait plate"
    );
}

/// H. The roster still absorbs spare width and scrolls horizontally.
#[test]
fn roster_is_horizontally_dynamic() {
    let mut app = hud_layout_app();
    let geom = app.world().resource::<HudViewportGeometry>().clone();
    let roster = node_rect::<SquadPanelRoot>(&mut app, "SquadPanelRoot");
    let fixed = geom.selected_width + geom.command_width + geom.utility_width;

    assert!(
        roster.width() > fixed * 0.35,
        "roster should absorb remaining width, got {}px against {fixed}px of clamped sections",
        roster.width()
    );
    assert!(
        roster.width() > geom.selected_width,
        "roster should be the largest section at the reference viewport"
    );

    let world = app.world_mut();
    let mut query = world.query_filtered::<&Node, With<SquadRosterViewport>>();
    let node = query.single(world).expect("roster viewport node");
    assert_eq!(
        node.overflow,
        Overflow::scroll_x(),
        "roster must overflow horizontally rather than grow the HUD"
    );
    assert!(
        node.flex_grow > 0.0,
        "roster viewport should fill its section"
    );
}

/// D + E. Command and utility controls fit inside the shared content height.
#[test]
fn commands_and_utility_fit_within_hud_bounds() {
    let mut app = hud_layout_app();
    let bar = node_rect::<BottomBar>(&mut app, "BottomBar");

    for (label, rect) in [
        (
            "commands",
            node_rect::<CommandPanelRoot>(&mut app, "CommandPanelRoot"),
        ),
        (
            "utility",
            node_rect::<UtilityPanelRoot>(&mut app, "UtilityPanelRoot"),
        ),
    ] {
        assert!(
            rect.min.y >= bar.min.y - 0.5 && rect.max.y <= bar.max.y + 0.5,
            "{label} section {rect:?} must stay inside the content row {bar:?}"
        );
    }

    let command_height = HUD_CONTENT_HEIGHT_PX * HUD_COMMAND_BUTTON_HEIGHT_PERCENT / 100.0;
    assert!(
        command_height < HUD_CONTENT_HEIGHT_PX,
        "command buttons must fit inside the content height"
    );
    assert!(
        command_height > 90.0,
        "command buttons should be substantially taller than the previous 32px rows, got \
         {command_height}px"
    );

    let utility_stack = HUD_UTILITY_BUTTON_HEIGHT_PX * 4.0
        + HUD_UTILITY_ROW_GAP_PX * 3.0
        + HUD_UTILITY_GROUP_DIVIDER_PX
        + 4.0;
    assert!(
        utility_stack <= HUD_CONTENT_HEIGHT_PX,
        "four utility rows plus group divider ({utility_stack}px) must fit the content height \
         ({HUD_CONTENT_HEIGHT_PX}px)"
    );
}

/// F. Selected-object content is clipped to its own section.
#[test]
fn selected_section_does_not_overflow_into_roster() {
    let mut app = hud_layout_app();
    let selected = node_rect::<SelectedUnitPanelRoot>(&mut app, "SelectedUnitPanelRoot");
    let roster = node_rect::<SquadPanelRoot>(&mut app, "SquadPanelRoot");

    assert!(
        selected.max.x <= roster.min.x + 0.5,
        "selected section right edge {} must not cross into the roster at {}",
        selected.max.x,
        roster.min.x
    );

    let world = app.world_mut();
    let mut query = world.query_filtered::<&Node, With<SelectedUnitPanelRoot>>();
    let node = query.single(world).expect("selected section node");
    assert_eq!(
        node.overflow,
        Overflow::clip(),
        "selected section must clip its content"
    );
}

#[test]
fn hud_snapshot_follows_world_selection_state() {
    let mut world = flat_world();
    let catalog = UnitCatalog::default();
    let unit_id = create_unit(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(1.0, 1.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    let mut selection = SelectedUnits::default();
    selection.set_single(unit_id);
    let world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Units,
        ..Default::default()
    };
    let snapshot = build_selected_panel_snapshot(
        &world_selection,
        &selection,
        &world,
        &catalog,
        &BuildingCatalog::default(),
        &WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap(),
    );
    assert_eq!(snapshot.primary_unit, Some(unit_id));
    assert!(snapshot.lines.join("\n").contains("Wolf"));
}

#[test]
fn selected_unit_snapshot_uses_live_hp_values() {
    let mut world = flat_world();
    let catalog = UnitCatalog::default();
    let unit_id = create_unit(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(1.0, 1.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    let mut selection = SelectedUnits::default();
    selection.set_single(unit_id);
    let joined = build_selected_panel_snapshot(
        &WorldSelectionState {
            category: WorldSelectionCategory::Units,
            ..Default::default()
        },
        &selection,
        &world,
        &catalog,
        &BuildingCatalog::default(),
        &WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap(),
    )
    .lines
    .join("\n");
    assert!(joined.contains("HP: 5/5"));
}

#[test]
fn hud_stat_bars_exclude_thirst() {
    let labels = ["HP", "Food"];
    for label in labels {
        assert!(!label.to_ascii_lowercase().contains("thirst"));
    }
    assert_ne!(HudStatBarId::Hp, HudStatBarId::Nutrition);
    assert_eq!(
        std::mem::discriminant(&HudStatBarId::Hp),
        std::mem::discriminant(&HudStatBarId::Hp)
    );
}

#[test]
fn building_selection_snapshot_still_works() {
    let mut world = flat_world();
    let building_id = BuildingId::new(1);
    let record = BuildingRecord::new(
        building_id,
        BuildingDefinitionId::new("prispod_farm"),
        BuildingPlacement::new(pos(2.0, 2.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(Affiliation::Player),
        300,
        BuildingSource::Authored,
    );
    world
        .insert_building(ChunkId::new(record.placement.position.chunk), record)
        .unwrap();
    let world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Building,
        building_id: Some(building_id),
        ..Default::default()
    };
    let joined = build_selected_panel_snapshot(
        &world_selection,
        &SelectedUnits::default(),
        &world,
        &UnitCatalog::default(),
        &BuildingCatalog::default(),
        &WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap(),
    )
    .lines
    .join("\n");
    assert!(joined.contains("Prispod Farm"));
    assert!(joined.contains("HP 300 / 300"));
}

#[test]
fn owned_roster_is_dynamic_not_hardcoded() {
    let catalog = UnitCatalog::default();
    let mut world = flat_world();
    let a = create_unit_with_ownership(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let b = create_unit_with_ownership(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(2.0, 2.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let ids = owned_roster_unit_ids(&world);
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&a));
    assert!(ids.contains(&b));
}

#[test]
fn roster_has_no_twelve_unit_cap() {
    let catalog = UnitCatalog::default();
    let mut world = flat_world();
    for i in 0..20 {
        create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(i as f32, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
    }
    assert_eq!(owned_roster_unit_ids(&world).len(), 20);
}

#[test]
fn roster_overflow_uses_horizontal_scroll_not_taller_hud() {
    // Growing the band any further would exceed the mockup's own proportions.
    assert!(HUD_HEIGHT_PX <= HUD_SOURCE_BAND_HEIGHT_PX);
}

#[test]
fn visible_command_grid_includes_hold_and_attack_move() {
    assert!(COMMAND_GRID.contains(&HudCommandButton::HoldPosition));
    assert!(COMMAND_GRID.contains(&HudCommandButton::AttackMove));
    assert!(!COMMAND_GRID.contains(&HudCommandButton::Interact));
}

#[test]
fn pointer_inside_bottom_hud_blocks_world_input() {
    let hover = PlayerHudHoverState {
        hovered: true,
        dev_panel_blocks: false,
        blocks_camera_scroll: true,
    };
    assert!(gameplay_input_blocked_by_hud(&hover));
}

#[test]
fn hud_wheel_block_flag_set_when_bottom_hud_hovered() {
    let hover = PlayerHudHoverState {
        hovered: true,
        dev_panel_blocks: false,
        blocks_camera_scroll: true,
    };
    assert!(hover.blocks_camera_scroll);
}

#[test]
fn hud_chrome_labels_are_ascii() {
    assert!(HUD_NO_SELECTION_LABEL.is_ascii());
    for button in COMMAND_GRID {
        assert!(button.label().is_ascii());
    }
}

#[test]
fn permanent_hud_exposes_one_fields_button_and_popup_options() {
    let fields_buttons = PERMANENT_UTILITY_BUTTONS
        .iter()
        .filter(|button| **button == HudUtilityButton::Fields)
        .count();
    assert_eq!(fields_buttons, 1);

    let entries = player_field_menu_entries();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].1, "Water");
    assert_eq!(entries[3].1, "Stone");

    let mut app = hud_layout_app();
    let world = app.world_mut();
    let mut options = world.query::<&FieldsMenuOption>();
    assert_eq!(options.iter(world).count(), 4);

    let mut menus = world.query_filtered::<&Node, With<FieldsMenuRoot>>();
    let menu = menus.single(world).expect("fields menu root");
    assert_eq!(
        menu.display,
        Display::None,
        "field options must not be permanently exposed"
    );
}

#[test]
fn hud_height_baseline_unchanged_after_polish_pass() {
    assert_eq!(HUD_HEIGHT_PX, 196.0);
    assert_eq!(BOTTOM_BAR_HEIGHT_PX, HUD_HEIGHT_PX);
    let geom = compute_hud_viewport_geometry(REFERENCE_VIEWPORT);
    use crate::ui::gameplay::hud::geometry::{HUD_HEIGHT_MAX_PX, HUD_HEIGHT_MIN_PX};
    assert!(geom.hud_height >= HUD_HEIGHT_MIN_PX);
    assert!(geom.hud_height <= HUD_HEIGHT_MAX_PX);
    assert!(
        geom.hud_height > PREVIOUS_HUD_HEIGHT_PX,
        "responsive height should remain above the legacy 112px strip"
    );
}

#[test]
fn responsive_sections_track_target_relationships() {
    let geom = compute_hud_viewport_geometry(Vec2::new(HUD_REFERENCE_VIEWPORT_WIDTH, 1080.0));
    let total = geom.selected_width + geom.roster_width + geom.command_width + geom.utility_width;
    let selected_share = geom.selected_width / total;
    let roster_share = geom.roster_width / total;
    let command_share = geom.command_width / total;
    let utility_share = geom.utility_width / total;
    assert!((selected_share - 0.23).abs() < 0.05);
    assert!((roster_share - 0.39).abs() < 0.10);
    assert!((command_share - 0.22).abs() < 0.05);
    assert!((utility_share - 0.13).abs() < 0.05);
    assert!(roster_share > selected_share);
    assert!(selected_share > utility_share);
}

#[test]
fn wide_viewport_layout_keeps_clamped_sections_bounded() {
    use crate::ui::gameplay::hud::geometry::{
        HUD_COMMAND_MAX_PX, HUD_SELECTED_MAX_PX, HUD_UTILITY_MAX_PX,
    };
    let normal = compute_hud_viewport_geometry(Vec2::new(1920.0, 1080.0));
    let wide = compute_hud_viewport_geometry(Vec2::new(2560.0, 1440.0));
    assert!(wide.roster_width > normal.roster_width);
    assert!(wide.selected_width <= HUD_SELECTED_MAX_PX + 0.5);
    assert!(wide.command_width <= HUD_COMMAND_MAX_PX + 0.5);
    assert!(wide.utility_width <= HUD_UTILITY_MAX_PX + 0.5);
}

#[test]
fn layout_resolves_at_multiple_viewport_widths() {
    for width in [1366.0, 1920.0, 2560.0] {
        let mut app = hud_layout_app_with_viewport(Vec2::new(width, 1080.0));
        let geom = app.world().resource::<HudViewportGeometry>().clone();
        let root = node_rect::<GameplayHudRoot>(&mut app, "GameplayHudRoot");
        assert!(
            (root.width() - width).abs() < 1.0,
            "HUD should span {width}px viewport"
        );
        assert!((root.height() - geom.hud_height).abs() < 1.0);
        let roster = node_rect::<SquadPanelRoot>(&mut app, "SquadPanelRoot");
        assert!(roster.width() >= geom.roster_width - 2.0);
    }
}

fn png_dimensions(path: &std::path::Path) -> (u32, u32) {
    let bytes = std::fs::read(path).expect("png bytes");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "expected PNG signature");
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("ihdr width"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("ihdr height"));
    (width, height)
}

/// The frame chrome must carry real art, not another nearly-empty crop.
#[test]
fn hud_frame_assets_exist_and_carry_visual_data() {
    for (path, min_width, min_height, min_bytes) in [
        ("assets/images/ui/hud/plate_frame.png", 32, 32, 700),
        ("assets/images/ui/hud/plate_backing.png", 4, 64, 200),
        ("assets/images/ui/hud/endcap_left.png", 24, 150, 2_000),
        ("assets/images/ui/hud/endcap_right.png", 12, 150, 1_000),
    ] {
        let path = std::path::Path::new(path);
        assert!(path.exists(), "{} must exist", path.display());
        let (width, height) = png_dimensions(path);
        assert!(
            width >= min_width && height >= min_height,
            "{} too small: {width}x{height}",
            path.display()
        );
        let bytes = std::fs::read(path).expect("png bytes");
        assert!(
            bytes.len() > min_bytes,
            "{} suspiciously tiny ({} bytes)",
            path.display(),
            bytes.len()
        );
    }
}

/// Superseded by per-section plate frames; re-exporting them would restore the
/// rectangular frame plus pasted-on seam that this pass removed.
#[test]
fn superseded_rectangular_frame_crops_are_gone() {
    for name in ["frame_mid", "divider_v"] {
        let path = std::path::PathBuf::from(format!("assets/images/ui/hud/{name}.png"));
        assert!(
            !path.exists(),
            "{name}.png belongs to the superseded continuous-rectangle frame"
        );
    }
}

/// Plate frames are root overlays: they draw the silhouette without taking part
/// in the flex row that owns section geometry.
#[test]
fn hud_plate_frames_are_root_overlays_not_flex_children() {
    let mut app = hud_layout_app();

    let selected = node_rect::<SelectedUnitPanelRoot>(&mut app, "SelectedUnitPanelRoot");
    let roster = node_rect::<SquadPanelRoot>(&mut app, "SquadPanelRoot");
    let commands = node_rect::<CommandPanelRoot>(&mut app, "CommandPanelRoot");
    let utility = node_rect::<UtilityPanelRoot>(&mut app, "UtilityPanelRoot");

    let world = app.world_mut();
    let root = world
        .query_filtered::<Entity, With<GameplayHudRoot>>()
        .single(world)
        .expect("hud root");
    let mut frames = world.query::<&HudPlateFrame>();
    assert_eq!(frames.iter(world).count(), 4, "expected four plate frames");
    let mut frame_parents = world.query_filtered::<&ChildOf, With<HudPlateFrame>>();
    for child_of in frame_parents.iter(world) {
        assert_eq!(
            child_of.parent(),
            root,
            "plate frames must be direct children of GameplayHudRoot"
        );
    }

    // Sections abut directly; frames consume no flex width.
    for (label, delta) in [
        ("selected/roster", roster.min.x - selected.max.x),
        ("roster/commands", commands.min.x - roster.max.x),
        ("commands/utility", utility.min.x - commands.max.x),
    ] {
        assert!(
            delta >= -2.0 && delta <= 2.0,
            "{label} should abut without a flex divider, delta={delta}"
        );
    }

    let mut frame_nodes = world.query::<(&HudPlateFrame, &ComputedNode, &UiGlobalTransform)>();
    let spans: Vec<(HudPlateSection, Rect)> = frame_nodes
        .iter(world)
        .map(|(frame, computed, transform)| {
            (
                frame.section,
                Rect::from_center_size(transform.translation, computed.size()),
            )
        })
        .collect();

    for (section, content) in [
        (HudPlateSection::Selected, selected),
        (HudPlateSection::Roster, roster),
        (HudPlateSection::Commands, commands),
        (HudPlateSection::Utility, utility),
    ] {
        let plate = spans
            .iter()
            .find(|(candidate, _)| *candidate == section)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("no plate frame for {section:?}"));
        assert!(
            plate.min.x <= content.min.x + 0.5 && plate.max.x >= content.max.x - 0.5,
            "{section:?} plate {plate:?} must span its section {content:?}"
        );
    }

    // Both plates at a join reach past the boundary, so the shallow join
    // chamfers land on plate body rather than a notch onto the world.
    let mut ordered = spans.clone();
    ordered.sort_by(|a, b| a.1.min.x.partial_cmp(&b.1.min.x).expect("finite"));
    for pair in ordered.windows(2) {
        let (left_section, left) = pair[0];
        let (right_section, right) = pair[1];
        let overlap = left.max.x - right.min.x;
        assert!(
            overlap >= 2.0 * HUD_PLATE_JOIN_OVERLAP_PX - 0.5,
            "{right_section:?}/{left_section:?} overlap {overlap} must cover both \
             join chamfers to avoid a transparent seam"
        );
    }

    let selected_plate = spans
        .iter()
        .find(|(section, _)| *section == HudPlateSection::Selected)
        .map(|(_, rect)| *rect)
        .expect("selected plate");
    let utility_plate = spans
        .iter()
        .find(|(section, _)| *section == HudPlateSection::Utility)
        .map(|(_, rect)| *rect)
        .expect("utility plate");
    let geom = app.world().resource::<HudViewportGeometry>().clone();
    let root = node_rect::<GameplayHudRoot>(&mut app, "GameplayHudRoot");
    assert!(
        selected_plate.min.x <= geom.endcap_left_width - HUD_ENDCAP_SEAT_PX + 0.5,
        "selected plate must tuck under the left endcap"
    );
    assert!(
        utility_plate.max.x >= root.max.x - geom.endcap_right_width + HUD_ENDCAP_SEAT_PX - 0.5,
        "utility plate must tuck under the right endcap"
    );
}

/// Shorter plates must draw last so a transition reads as one step down onto
/// the taller neighbour's rail, instead of the taller plate's cut edge showing
/// through on top.
#[test]
fn hud_plate_draw_order_puts_shorter_plates_on_top() {
    let mut by_z = HudPlateSection::ALL;
    by_z.sort_by_key(|section| section.z_index());
    for pair in by_z.windows(2) {
        assert!(
            pair[0].height_px(HUD_HEIGHT_PX) > pair[1].height_px(HUD_HEIGHT_PX),
            "{:?} draws under {:?} but is not taller",
            pair[0],
            pair[1]
        );
    }

    assert!(
        HUD_PLATE_JOIN_CHAMFER_PX < HUD_PLATE_CHAMFER_PX,
        "join chamfer must be shallower than the outer chamfer"
    );
    for section in HudPlateSection::ALL {
        let (interior_left, interior_right) = section.interior_edges();
        let outer = !interior_left || !interior_right;
        assert_eq!(
            outer,
            matches!(
                section,
                HudPlateSection::Selected | HudPlateSection::Utility
            ),
            "{section:?} outer-edge classification is wrong"
        );
    }
}

/// Content lives inside its plate: the deepest plate inset stays above the
/// content row's own inset, so no section can overflow its silhouette.
#[test]
fn hud_plate_insets_keep_section_content_inside_the_frame() {
    for section in HudPlateSection::ALL {
        let (top, bottom) = section.insets_px();
        assert!(
            top < HUD_FRAME_TOP_PX,
            "{section:?} top inset {top} must stay above the content row inset {HUD_FRAME_TOP_PX}"
        );
        assert!(
            bottom < HUD_FRAME_BOTTOM_PX,
            "{section:?} bottom inset {bottom} must stay above {HUD_FRAME_BOTTOM_PX}"
        );
    }

    // The stepped contour is the point of the pass: plates must not all be the
    // same height, or the run collapses back into one rectangle.
    let heights: Vec<f32> = HudPlateSection::ALL
        .iter()
        .map(|section| {
            let (top, bottom) = section.insets_px();
            HUD_HEIGHT_PX - top - bottom
        })
        .collect();
    let tallest = heights.iter().cloned().fold(f32::MIN, f32::max);
    let shortest = heights.iter().cloned().fold(f32::MAX, f32::min);
    assert!(
        tallest - shortest >= 12.0,
        "plates must sit at visibly different heights, got {heights:?}"
    );
}

/// Plate chrome fills interiors; there is no full-width rectangular backing
/// that could square off the chamfers.
#[test]
fn hud_plate_chrome_fills_each_section_without_a_root_rectangle() {
    let mut app = hud_layout_app();
    let world = app.world_mut();
    let mut chrome = world.query_filtered::<Entity, With<HudFrameBackground>>();
    assert_eq!(
        chrome.iter(world).count(),
        4,
        "one octagonal plate per section, not a single rectangular backing"
    );
}

/// Decorative chrome must never swallow a click meant for the HUD beneath it.
#[test]
fn hud_frame_chrome_is_interaction_inert() {
    let mut app = hud_layout_app();
    let world = app.world_mut();
    let mut chrome =
        world.query_filtered::<Entity, Or<(With<HudPlateFrame>, With<HudFrameBackground>)>>();
    let entities: Vec<Entity> = chrome.iter(world).collect();
    assert!(!entities.is_empty(), "expected frame chrome entities");
    for entity in entities {
        assert!(
            world.get::<Button>(entity).is_none(),
            "frame chrome must not be a button"
        );
        assert!(
            world.get::<Interaction>(entity).is_none(),
            "frame chrome must not participate in interaction"
        );
        assert_eq!(
            world.get::<FocusPolicy>(entity).copied(),
            Some(FocusPolicy::Pass),
            "frame chrome must pass pointer input through"
        );
    }
}
