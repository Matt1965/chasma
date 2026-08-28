//! Debug window panel — overlay toggles and animation diagnostics (Slice 8).

use bevy::prelude::*;

use crate::dev::DevModeInputGate;
use crate::dev::catalog_cache::DevSearchDebounce;
use crate::dev::dev_mode::{DevDebugFlags, DevModeState};
use crate::dev::input::DevPanelUi;
use crate::dev::widgets::{
    DevCollapsibleSectionId, DevWidgetToggle, DevWidgetToggleMark, spawn_toggle_row,
    sync_toggle_styles_with_marker,
    theme::{TEXT_SECTION, small_text_font},
};
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};

use crate::dev::tooltip::DevTooltipContent;

#[derive(Component, Debug)]
pub(crate) struct DevDebugWindowUi;

#[derive(Component, Debug)]
pub(crate) struct DevDebugSummaryText;

#[derive(Component, Debug)]
pub struct DevAnimationText;

#[derive(Component, Debug)]
pub(crate) struct DevDebugToggleButton {
    pub flag: DevDebugToggleFlag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevDebugToggleFlag {
    Master,
    Paths,
    Steering,
    Formations,
    Selection,
    Interaction,
    Combat,
    RelationshipLinks,
    Health,
    CommandTrace,
    NavWalkable,
    NavBlockers,
    NavFootprints,
    NavEntrances,
    NavReservations,
    NavOccupancy,
    NavBlueprint,
    ResetDevState,
}

struct ToggleDef {
    label: &'static str,
    flag: DevDebugToggleFlag,
    tooltip: &'static str,
}

const GROUP_TEXT: Color = Color::srgba(0.65, 0.78, 0.88, 1.0);

struct ToggleGroup {
    section: DevCollapsibleSectionId,
    title: &'static str,
    toggles: &'static [ToggleDef],
}

const TOGGLE_GROUPS: &[ToggleGroup] = &[
    ToggleGroup {
        section: DevCollapsibleSectionId::DebugMaster,
        title: "Master and general overlays",
        toggles: &[
            ToggleDef {
                label: "Master overlay",
                flag: DevDebugToggleFlag::Master,
                tooltip: "Global gate for dev overlay drawing. Sub-toggles keep their values while \
                          master is off; re-enabling restores them. Does not affect simulation.",
            },
            ToggleDef {
                label: "Paths",
                flag: DevDebugToggleFlag::Paths,
                tooltip: "Draws active unit path polylines from pathfinding results. Requires \
                          master overlay. Moderate cost when many units path.",
            },
            ToggleDef {
                label: "Steering",
                flag: DevDebugToggleFlag::Steering,
                tooltip: "Draws steering forces and desired velocity vectors. Requires master \
                          overlay. Diagnostic only.",
            },
            ToggleDef {
                label: "Formations",
                flag: DevDebugToggleFlag::Formations,
                tooltip: "Draws formation slot offsets and facing. Requires master overlay.",
            },
        ],
    },
    ToggleGroup {
        section: DevCollapsibleSectionId::DebugSelection,
        title: "Selection and inspector focus",
        toggles: &[
            ToggleDef {
                label: "Selection gizmos",
                flag: DevDebugToggleFlag::Selection,
                tooltip: "Highlights shared world selection (units, buildings, doodads). Focus \
                          follows Selected Object / inspector, not a separate debug selection.",
            },
            ToggleDef {
                label: "Interaction hits",
                flag: DevDebugToggleFlag::Interaction,
                tooltip: "Draws interaction query hits and ranges. Requires master overlay.",
            },
            ToggleDef {
                label: "Combat overlay",
                flag: DevDebugToggleFlag::Combat,
                tooltip: "Draws combat ranges and engagement diagnostics. Requires master overlay.",
            },
            ToggleDef {
                label: "Relationship Links",
                flag: DevDebugToggleFlag::RelationshipLinks,
                tooltip: "Draws mutual-perception relationship links between nearby units. \
                          Reads perception + relationship authorities only; does not change \
                          simulation. Requires master overlay.",
            },
            ToggleDef {
                label: "Health bars (all)",
                flag: DevDebugToggleFlag::Health,
                tooltip: "Forces health bars on all damageable entities. Higher visual cost; \
                          requires master overlay.",
            },
            ToggleDef {
                label: "Command trace",
                flag: DevDebugToggleFlag::CommandTrace,
                tooltip: "Shows recent intent/command trace for selected units. Requires master \
                          overlay.",
            },
        ],
    },
    ToggleGroup {
        section: DevCollapsibleSectionId::DebugNavigation,
        title: "Navigation (NV0)",
        toggles: &[
            ToggleDef {
                label: "Navigation/Pathing Mask",
                flag: DevDebugToggleFlag::NavWalkable,
                tooltip: "Whole-world navigable (green) and blocked (reason-colored) navigation \
                          cells from authoritative passability. No selection required. Requires \
                          master overlay. Does not change pathfinding.",
            },
            ToggleDef {
                label: "Blocked Area",
                flag: DevDebugToggleFlag::NavBlockers,
                tooltip: "Filled cells showing actual movement authority blocking \
                          (blueprint boundaries or legacy footprints). Requires master overlay.",
            },
            ToggleDef {
                label: "Nav footprints",
                flag: DevDebugToggleFlag::NavFootprints,
                tooltip: "Building footprint outlines used for nav blocking. Requires master overlay.",
            },
            ToggleDef {
                label: "Nav entrances",
                flag: DevDebugToggleFlag::NavEntrances,
                tooltip: "Entrance markers on building navigation blueprints. Requires master overlay.",
            },
            ToggleDef {
                label: "Nav reservations",
                flag: DevDebugToggleFlag::NavReservations,
                tooltip: "Reserved nav cells for active movement. Requires master overlay.",
            },
            ToggleDef {
                label: "Nav occupancy",
                flag: DevDebugToggleFlag::NavOccupancy,
                tooltip: "Occupied nav cells from units and obstacles. Requires master overlay.",
            },
            ToggleDef {
                label: "Nav blueprint",
                flag: DevDebugToggleFlag::NavBlueprint,
                tooltip: "Resolved navigation blueprint mesh overlay for inspected buildings. \
                          Requires master overlay.",
            },
        ],
    },
    ToggleGroup {
        section: DevCollapsibleSectionId::DebugSession,
        title: "Session utilities",
        toggles: &[ToggleDef {
            label: "Reset dev state",
            flag: DevDebugToggleFlag::ResetDevState,
            tooltip: "Clears catalog search, placement tool, and tool state. Does not reset overlay \
                      flags, lighting, or field data. Dev mode stays enabled.",
        }],
    },
];

pub fn setup_debug_window_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::Debug {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevDebugWindowUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    root.spawn((
                        DevDebugSummaryText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.72, 0.82, 0.9, 1.0)),
                    ));
                    for group in TOGGLE_GROUPS {
                        root.spawn((
                            DevPanelUi,
                            Text::new(group.title),
                            small_text_font(),
                            TextColor(TEXT_SECTION),
                        ));
                        for toggle in group.toggles {
                            spawn_toggle_row(
                                root,
                                toggle.label,
                                DevTooltipContent::new(toggle.tooltip),
                                DevDebugToggleButton { flag: toggle.flag },
                            );
                        }
                    }
                    root.spawn((
                        DevPanelUi,
                        Text::new("Animation diagnostics"),
                        small_text_font(),
                        TextColor(TEXT_SECTION),
                    ));
                    root.spawn((
                        DevAnimationText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.72, 0.82, 0.9, 1.0)),
                    ));
                });
        });
        return;
    }
}

pub fn sync_debug_panel_content(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mask_stats: Option<Res<crate::debug::NavigationMaskDrawStats>>,
    mut summary: Query<&mut Text, With<DevDebugSummaryText>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Debug) {
        return;
    }
    let Ok(mut text) = summary.single_mut() else {
        return;
    };
    **text = format_debug_summary(
        &dev_state.debug_config,
        mask_stats.as_deref().copied().unwrap_or_default(),
    );
}

pub fn sync_debug_panel_button_styles(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut buttons: Query<
        (
            &Interaction,
            &DevWidgetToggle,
            &DevDebugToggleButton,
            &mut BackgroundColor,
            &Children,
        ),
        With<Button>,
    >,
    mut marks: Query<&mut Visibility, With<DevWidgetToggleMark>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Debug) {
        return;
    }
    let config = dev_state.debug_config;
    sync_toggle_styles_with_marker(
        |toggle: &DevDebugToggleButton| match toggle.flag {
            DevDebugToggleFlag::Master => config.enabled,
            DevDebugToggleFlag::Paths => config.path,
            DevDebugToggleFlag::Steering => config.steering,
            DevDebugToggleFlag::Formations => config.formation,
            DevDebugToggleFlag::Selection => config.selection,
            DevDebugToggleFlag::Interaction => config.interaction,
            DevDebugToggleFlag::Combat => config.combat,
            DevDebugToggleFlag::RelationshipLinks => config.relationship_links,
            DevDebugToggleFlag::Health => config.health,
            DevDebugToggleFlag::CommandTrace => config.intent,
            DevDebugToggleFlag::NavWalkable => config.grid,
            DevDebugToggleFlag::NavBlockers => config.nav_blockers,
            DevDebugToggleFlag::NavFootprints => config.nav_footprints,
            DevDebugToggleFlag::NavEntrances => config.nav_entrances,
            DevDebugToggleFlag::NavReservations => config.nav_reservations,
            DevDebugToggleFlag::NavOccupancy => config.nav_occupancy,
            DevDebugToggleFlag::NavBlueprint => config.nav_blueprint,
            DevDebugToggleFlag::ResetDevState => false,
        },
        buttons,
        marks,
    );
}

pub fn handle_debug_toggle_buttons(
    registry: Res<DevWindowRegistry>,
    mut dev_state: ResMut<DevModeState>,
    mut debounce: ResMut<DevSearchDebounce>,
    mut gate: ResMut<DevModeInputGate>,
    buttons: Query<(&Interaction, &DevDebugToggleButton), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Debug) {
        return;
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        toggle_debug_flag(&mut dev_state, button.flag, &mut debounce);
    }
}

fn toggle_debug_flag(
    state: &mut DevModeState,
    flag: DevDebugToggleFlag,
    debounce: &mut DevSearchDebounce,
) {
    match flag {
        DevDebugToggleFlag::Master => state.debug_config.enabled = !state.debug_config.enabled,
        DevDebugToggleFlag::Paths => state.debug_config.path = !state.debug_config.path,
        DevDebugToggleFlag::Steering => state.debug_config.steering = !state.debug_config.steering,
        DevDebugToggleFlag::Formations => {
            state.debug_config.formation = !state.debug_config.formation
        }
        DevDebugToggleFlag::Selection => {
            state.debug_config.selection = !state.debug_config.selection
        }
        DevDebugToggleFlag::Interaction => {
            state.debug_config.interaction = !state.debug_config.interaction
        }
        DevDebugToggleFlag::Combat => state.debug_config.combat = !state.debug_config.combat,
        DevDebugToggleFlag::RelationshipLinks => {
            state.debug_config.relationship_links = !state.debug_config.relationship_links
        }
        DevDebugToggleFlag::Health => state.debug_config.health = !state.debug_config.health,
        DevDebugToggleFlag::CommandTrace => state.debug_config.intent = !state.debug_config.intent,
        DevDebugToggleFlag::NavWalkable => state.debug_config.grid = !state.debug_config.grid,
        DevDebugToggleFlag::NavBlockers => {
            state.debug_config.nav_blockers = !state.debug_config.nav_blockers
        }
        DevDebugToggleFlag::NavFootprints => {
            state.debug_config.nav_footprints = !state.debug_config.nav_footprints
        }
        DevDebugToggleFlag::NavEntrances => {
            state.debug_config.nav_entrances = !state.debug_config.nav_entrances
        }
        DevDebugToggleFlag::NavReservations => {
            state.debug_config.nav_reservations = !state.debug_config.nav_reservations
        }
        DevDebugToggleFlag::NavOccupancy => {
            state.debug_config.nav_occupancy = !state.debug_config.nav_occupancy
        }
        DevDebugToggleFlag::NavBlueprint => {
            state.debug_config.nav_blueprint = !state.debug_config.nav_blueprint
        }
        DevDebugToggleFlag::ResetDevState => {
            let enabled = state.enabled;
            let active_tab = state.active_tab;
            state.reset_tool_state();
            state.enabled = enabled;
            state.active_tab = active_tab;
            debounce.note_input(&state.search_query);
        }
    }
}

pub(crate) fn format_debug_summary(
    flags: &DevDebugFlags,
    mask_stats: crate::debug::NavigationMaskDrawStats,
) -> String {
    let mut summary = format!(
        "Overlay master: {}\nPaths: {}  Steering: {}  Formations: {}\nSelection: {}  Interaction: {}  Combat: {}  RelLinks: {}  Health: {}  Trace: {}\nPathing mask: {}  Block: {}  Foot: {}  Entr: {}  Rsv: {}  Occ: {}  Blueprint: {}",
        flags.enabled,
        flags.path,
        flags.steering,
        flags.formation,
        flags.selection,
        flags.interaction,
        flags.combat,
        flags.relationship_links,
        flags.health,
        flags.intent,
        flags.grid,
        flags.nav_blockers,
        flags.nav_footprints,
        flags.nav_entrances,
        flags.nav_reservations,
        flags.nav_occupancy,
        flags.nav_blueprint,
    );
    if flags.enabled && flags.grid {
        summary.push_str(&format!(
            "\nMask legend: green=navigable  orange=slope  red=building  purple=doodad  gray=other\nMask draw: sampled={}  navigable={}  blocked={}  ran={}",
            mask_stats.cells_sampled,
            mask_stats.navigable_drawn,
            mask_stats.blocked_drawn,
            mask_stats.ran,
        ));
    }
    summary
}

/// Count of live Debug toggle rows declared in [`TOGGLE_GROUPS`].
#[cfg(test)]
pub fn expected_debug_toggle_button_count() -> usize {
    TOGGLE_GROUPS.iter().map(|group| group.toggles.len()).sum()
}

/// Every `(label, flag)` pair that must produce a [`DevDebugToggleButton`].
#[cfg(test)]
pub fn debug_toggle_defs_for_tests() -> Vec<(&'static str, DevDebugToggleFlag)> {
    TOGGLE_GROUPS
        .iter()
        .flat_map(|group| {
            group
                .toggles
                .iter()
                .map(|toggle| (toggle.label, toggle.flag))
        })
        .collect()
}

#[cfg(test)]
mod panel_construction_tests {
    use super::*;
    use crate::debug::DebugOverlayConfig;
    use crate::dev::DevModeInputGate;
    use crate::dev::catalog_cache::DevSearchDebounce;
    use crate::dev::dev_mode::DevModeState;
    use crate::dev::window::{DevWindowId, DevWindowRegistry, setup_dev_workspace};
    use bevy::ecs::system::RunSystemOnce;

    fn headless_debug_ui_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
        app.init_resource::<DevWindowRegistry>()
            .init_resource::<DevModeState>()
            .init_resource::<DevSearchDebounce>()
            .init_resource::<DevModeInputGate>()
            .init_resource::<crate::dev::widgets::DevCollapsibleState>();
        app
    }

    fn relationship_links_toggle_entity(world: &mut World) -> Entity {
        world
            .query::<(Entity, &DevDebugToggleButton)>()
            .iter(world)
            .find(|(_, toggle)| toggle.flag == DevDebugToggleFlag::RelationshipLinks)
            .map(|(entity, _)| entity)
            .expect("Relationship Links DevDebugToggleButton")
    }

    #[test]
    fn format_debug_summary_includes_relationship_links() {
        let mut config = DebugOverlayConfig::production();
        config.relationship_links = true;
        let summary =
            format_debug_summary(&config, crate::debug::NavigationMaskDrawStats::default());
        assert!(summary.contains("RelLinks: true"));
    }

    #[test]
    fn debug_panel_spawns_one_button_per_toggle_def() {
        let mut app = headless_debug_ui_app();
        app.world_mut()
            .run_system_once(setup_dev_workspace)
            .expect("setup_dev_workspace");
        app.world_mut()
            .run_system_once(setup_debug_window_panel)
            .expect("setup_debug_window_panel");

        let mut world = app.world_mut();
        let buttons: Vec<_> = world
            .query::<&DevDebugToggleButton>()
            .iter(&mut world)
            .collect();
        assert_eq!(buttons.len(), expected_debug_toggle_button_count());
        assert_eq!(
            buttons
                .iter()
                .filter(|toggle| toggle.flag == DevDebugToggleFlag::RelationshipLinks)
                .count(),
            1
        );
    }

    #[test]
    fn relationship_links_toggle_is_a_button_with_label() {
        let mut app = headless_debug_ui_app();
        app.world_mut()
            .run_system_once(setup_dev_workspace)
            .expect("setup_dev_workspace");
        app.world_mut()
            .run_system_once(setup_debug_window_panel)
            .expect("setup_debug_window_panel");

        let entity = relationship_links_toggle_entity(app.world_mut());
        let world = app.world();
        assert!(world.get::<Button>(entity).is_some());
        assert!(world.get::<DevWidgetToggle>(entity).is_some());

        let mut world = app.world_mut();
        let labels: Vec<String> = world
            .query::<&Text>()
            .iter(&mut world)
            .map(|text| text.to_string())
            .collect();
        assert!(
            labels.iter().any(|label| label == "Relationship Links"),
            "expected label text among spawned Debug panel strings"
        );
        assert!(
            labels
                .iter()
                .any(|label| label == "Selection and inspector focus"),
            "expected flat section header among spawned Debug panel strings"
        );
    }

    #[test]
    fn toggle_def_inventory_matches_spawned_buttons() {
        let defs = debug_toggle_defs_for_tests();
        let mut app = headless_debug_ui_app();
        app.world_mut()
            .run_system_once(setup_dev_workspace)
            .expect("setup_dev_workspace");
        app.world_mut()
            .run_system_once(setup_debug_window_panel)
            .expect("setup_debug_window_panel");

        let mut world = app.world_mut();
        let mut spawned: Vec<DevDebugToggleFlag> = world
            .query::<&DevDebugToggleButton>()
            .iter(&mut world)
            .map(|toggle| toggle.flag)
            .collect();
        spawned.sort_by_key(|flag| format!("{flag:?}"));

        let mut expected: Vec<DevDebugToggleFlag> = defs.iter().map(|(_, flag)| *flag).collect();
        expected.sort_by_key(|flag| format!("{flag:?}"));

        assert_eq!(spawned, expected);
    }

    #[test]
    fn pressing_relationship_links_toggle_flips_backing_state() {
        let mut app = headless_debug_ui_app();
        app.world_mut()
            .run_system_once(setup_dev_workspace)
            .expect("setup_dev_workspace");
        app.world_mut()
            .run_system_once(setup_debug_window_panel)
            .expect("setup_debug_window_panel");

        let button = relationship_links_toggle_entity(app.world_mut());
        {
            let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
            registry.show(DevWindowId::Debug);
            let mut dev_state = app.world_mut().resource_mut::<DevModeState>();
            dev_state.enabled = true;
            assert!(!dev_state.debug_config.relationship_links);
        }

        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.world_mut()
            .run_system_once(handle_debug_toggle_buttons)
            .expect("handle_debug_toggle_buttons");

        assert!(
            app.world()
                .resource::<DevModeState>()
                .debug_config
                .relationship_links
        );
    }

    #[test]
    fn relationship_links_checked_state_syncs_toggle_mark() {
        let mut app = headless_debug_ui_app();
        app.world_mut()
            .run_system_once(setup_dev_workspace)
            .expect("setup_dev_workspace");
        app.world_mut()
            .run_system_once(setup_debug_window_panel)
            .expect("setup_debug_window_panel");

        let button = relationship_links_toggle_entity(app.world_mut());
        {
            let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
            registry.show(DevWindowId::Debug);
            let mut dev_state = app.world_mut().resource_mut::<DevModeState>();
            dev_state.enabled = true;
            dev_state.debug_config.relationship_links = true;
        }

        app.world_mut()
            .run_system_once(sync_debug_panel_button_styles)
            .expect("sync_debug_panel_button_styles");

        let children = app
            .world()
            .get::<Children>(button)
            .expect("toggle children");
        let mark_entity = children.iter().next().expect("toggle mark child");
        assert_eq!(
            *app.world()
                .get::<Visibility>(mark_entity)
                .expect("mark visibility"),
            Visibility::Visible
        );
    }
}
