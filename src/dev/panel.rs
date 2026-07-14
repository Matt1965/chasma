//! Dev mode panel UI (Bevy UI, ADR-043).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::world::{
    BuildingCatalog, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, UnitCatalog,
};

use super::catalog_browser::CatalogBrowserEntry;
use super::catalog_cache::{
    CatalogBrowseIndex, CatalogFilterCache, DevSearchDebounce, browse_catalog_entries,
};
use super::dev_mode::{DevDebugFlags, DevModeState, DevTab};
use super::input::{DevPanelRoot, DevPanelUi};
use super::scenes::{
    DevSceneRegistry, SceneDebugFlagsSnapshot, clear_dev_world, delete_scene, load_scene_by_id,
    save_current_world,
};
use super::tools::MAX_BRUSH_SPAWN_COUNT;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::doodads::DoodadsRuntimeSettings;
use crate::simulation::{SimulationControlRequests, SimulationControlState};
use crate::world::WorldData;

const MAX_VISIBLE_ROWS: usize = 12;
const ROW_HEIGHT_PX: f32 = 22.0;
const PANEL_WIDTH_PX: f32 = 368.0;
const MENU_BTN_WIDTH_PX: f32 = 100.0;
const MENU_BTN_HEIGHT_PX: f32 = 24.0;
const TAB_BTN_WIDTH_PX: f32 = 50.0;
const MAX_LIST_LABEL_CHARS: usize = 44;

const BTN_BG_IDLE: Color = Color::srgba(0.14, 0.22, 0.28, 0.95);
const BTN_BG_HOVER: Color = Color::srgba(0.20, 0.30, 0.38, 0.98);
const BTN_BG_PRESSED: Color = Color::srgba(0.08, 0.12, 0.16, 1.0);
const BTN_BG_ACTIVE: Color = Color::srgba(0.15, 0.45, 0.32, 0.95);
const SEARCH_BG_IDLE: Color = Color::srgba(0.08, 0.11, 0.14, 0.95);
const SEARCH_BG_FOCUSED: Color = Color::srgba(0.10, 0.18, 0.24, 0.98);
const SEARCH_BORDER_IDLE: Color = Color::srgba(0.25, 0.32, 0.38, 0.9);
const SEARCH_BORDER_FOCUSED: Color = Color::srgba(0.35, 0.75, 0.55, 1.0);

fn menu_button_bg(interaction: &Interaction, selected: bool) -> BackgroundColor {
    if selected {
        return BackgroundColor(BTN_BG_ACTIVE);
    }
    BackgroundColor(match interaction {
        Interaction::Pressed => BTN_BG_PRESSED,
        Interaction::Hovered => BTN_BG_HOVER,
        Interaction::None => BTN_BG_IDLE,
    })
}

#[derive(Component, Debug)]
pub(crate) struct DevPanelTitle;

#[derive(Component, Debug)]
pub(crate) struct DevSimulationStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevSimulationAction {
    TogglePause,
    StepOnce,
}

#[derive(Component, Debug)]
pub(crate) struct DevSimulationButton {
    action: DevSimulationAction,
}

#[derive(Component, Debug)]
pub(crate) struct DevSearchText;

#[derive(Component, Debug)]
pub(crate) struct DevSearchBox;

#[derive(Component, Debug)]
pub(crate) struct DevSearchClearButton;

#[derive(Component, Debug)]
pub(crate) struct DevToolStatusText;

#[derive(Component, Debug)]
pub(crate) struct DevListText;

#[derive(Component, Debug)]
pub(crate) struct DevSpawnHintText;

#[derive(Component, Debug)]
pub(crate) struct DevDebugText;

#[derive(Component, Debug)]
pub(crate) struct DevAnimationText;

#[derive(Component, Debug)]
pub(crate) struct DevWorldToolsText;

#[derive(Component, Debug)]
pub(crate) struct DevPlacementText;

#[derive(Component, Debug)]
pub(crate) struct DevScenesSection;

#[derive(Component, Debug)]
pub(crate) struct DevScenesText;

#[derive(Component, Debug)]
pub(crate) struct DevSceneNameText;

#[derive(Component, Debug)]
pub(crate) struct DevSceneButton {
    action: DevSceneAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevSceneAction {
    SaveCurrent,
    ReloadLast,
    ClearWorld,
    DeleteSelected,
}

#[derive(Component, Debug)]
pub(crate) struct DevCatalogSection;

#[derive(Component, Debug)]
pub(crate) struct DevPlacementSection;

#[derive(Component, Debug)]
pub(crate) struct DevPlacementButton {
    action: DevPlacementAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevPlacementAction {
    CycleBrush,
    CountUp,
    CountDown,
    SpacingUp,
    SpacingDown,
    RadiusUp,
    RadiusDown,
    ToggleTerrainSnap,
    TogglePreview,
    CycleSpawnTeam,
}

#[derive(Component, Debug)]
pub(crate) struct DevTabButton {
    tab: DevTab,
}

#[derive(Component, Debug)]
pub(crate) struct DevListRow {
    index: usize,
}

#[derive(Component, Debug)]
pub(crate) struct DevToggleButton {
    flag: DevToggleFlag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevToggleFlag {
    EnabledOnly,
    Master,
    Paths,
    Steering,
    Formations,
    Selection,
    Interaction,
    Combat,
    Health,
    CommandTrace,
    Grid,
    ResetDevState,
}

/// Spawn the dev panel tree once at startup (hidden until F12).
pub(crate) fn setup_dev_panel(mut commands: Commands) {
    commands
        .spawn((
            DevPanelRoot,
            DevPanelUi,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(PANEL_WIDTH_PX),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.08, 0.92)),
            FocusPolicy::Block,
            ZIndex(900),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                DevPanelTitle,
                DevPanelUi,
                Text::new("DEV MODE (F12)"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgba(0.35, 0.95, 0.55, 1.0)),
            ));

            root.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
            ))
            .with_children(|tabs| {
                for tab in [
                    DevTab::Units,
                    DevTab::Doodads,
                    DevTab::Buildings,
                    DevTab::Placement,
                    DevTab::Scenes,
                    DevTab::Inspector,
                    DevTab::Debug,
                    DevTab::WorldTools,
                ] {
                    tabs.spawn((
                        DevTabButton { tab },
                        DevPanelUi,
                        Button,
                        Node {
                            width: Val::Px(TAB_BTN_WIDTH_PX),
                            height: Val::Px(MENU_BTN_HEIGHT_PX),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            padding: UiRect::ZERO,
                            ..default()
                        },
                        BackgroundColor(BTN_BG_IDLE),
                        Text::new(tab_label(tab)),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
                    ));
                }
            });

            root.spawn((
                DevSimulationStatus,
                DevPanelUi,
                Text::new("Sim: running   tick      0"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.88, 0.95, 1.0)),
                Node {
                    min_height: Val::Px(14.0),
                    ..default()
                },
            ));

            root.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    min_height: Val::Px(MENU_BTN_HEIGHT_PX),
                    ..default()
                },
            ))
            .with_children(|row| {
                for (label, action) in [
                    ("Pause/Resume", DevSimulationAction::TogglePause),
                    ("Step tick", DevSimulationAction::StepOnce),
                ] {
                    row.spawn((
                        DevSimulationButton { action },
                        DevPanelUi,
                        Button,
                        Node {
                            width: Val::Px(MENU_BTN_WIDTH_PX),
                            height: Val::Px(MENU_BTN_HEIGHT_PX),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            padding: UiRect::ZERO,
                            ..default()
                        },
                        BackgroundColor(BTN_BG_IDLE),
                        Text::new(label),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
                    ));
                }
            });

            root.spawn((
                DevToolStatusText,
                DevPanelUi,
                Text::new("Tool: none"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.70, 0.88, 0.78, 1.0)),
                Node {
                    min_height: Val::Px(56.0),
                    ..default()
                },
            ));

            root.spawn((
                DevCatalogSection,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
            ))
            .with_children(|catalog| {
                catalog
                    .spawn((
                        DevSearchBox,
                        DevPanelUi,
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(MENU_BTN_HEIGHT_PX),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(4.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(SEARCH_BG_IDLE),
                        BorderColor::all(SEARCH_BORDER_IDLE),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            DevSearchText,
                            DevPanelUi,
                            Text::new("Search definitions… (Ctrl+F or /)"),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.65, 0.72, 0.80, 1.0)),
                            Node {
                                flex_grow: 1.0,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                        ));
                        row.spawn((
                            DevSearchClearButton,
                            DevPanelUi,
                            Button,
                            Node {
                                width: Val::Px(20.0),
                                height: Val::Px(20.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            Visibility::Hidden,
                            BackgroundColor(Color::srgba(0.18, 0.24, 0.30, 0.9)),
                            Text::new("×"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.85, 0.90, 0.95, 1.0)),
                        ));
                    });

                catalog.spawn((
                    DevListText,
                    DevPanelUi,
                    Text::new(""),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.9, 0.93, 0.96, 1.0)),
                ));

                catalog
                    .spawn((
                        DevPanelUi,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            max_height: Val::Px(ROW_HEIGHT_PX * MAX_VISIBLE_ROWS as f32),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        for index in 0..MAX_VISIBLE_ROWS {
                            list.spawn((
                                DevListRow { index },
                                DevPanelUi,
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(ROW_HEIGHT_PX),
                                    padding: UiRect::horizontal(Val::Px(4.0)),
                                    align_items: AlignItems::Center,
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.1, 0.14, 0.18, 0.85)),
                                Text::new(""),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgba(0.88, 0.92, 0.96, 1.0)),
                            ));
                        }
                    });
            });

            root.spawn((
                DevPlacementSection,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|placement| {
                placement.spawn((
                    DevPlacementText,
                    DevPanelUi,
                    Text::new("Placement tools"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
                ));
                for (label, action) in [
                    ("Brush mode", DevPlacementAction::CycleBrush),
                    ("Count +", DevPlacementAction::CountUp),
                    ("Count -", DevPlacementAction::CountDown),
                    ("Spacing +", DevPlacementAction::SpacingUp),
                    ("Spacing -", DevPlacementAction::SpacingDown),
                    ("Radius +", DevPlacementAction::RadiusUp),
                    ("Radius -", DevPlacementAction::RadiusDown),
                    ("Toggle terrain snap", DevPlacementAction::ToggleTerrainSnap),
                    ("Toggle preview", DevPlacementAction::TogglePreview),
                    ("Cycle spawn team", DevPlacementAction::CycleSpawnTeam),
                ] {
                    placement.spawn((
                        DevPlacementButton { action },
                        DevPanelUi,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.12, 0.2, 0.28, 0.95)),
                        Text::new(label),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
                    ));
                }
            });

            root.spawn((
                DevScenesSection,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|scenes| {
                scenes.spawn((
                    DevScenesText,
                    DevPanelUi,
                    Text::new("Scene tools"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
                ));
                scenes.spawn((
                    DevSceneNameText,
                    DevPanelUi,
                    Text::new("Name: Untitled Scene"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.82, 0.9, 1.0)),
                ));
                for (label, action) in [
                    ("Save Current World", DevSceneAction::SaveCurrent),
                    ("Reload Last Scene", DevSceneAction::ReloadLast),
                    ("Clear World", DevSceneAction::ClearWorld),
                    ("Delete Scene", DevSceneAction::DeleteSelected),
                ] {
                    scenes.spawn((
                        DevSceneButton { action },
                        DevPanelUi,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.12, 0.2, 0.28, 0.95)),
                        Text::new(label),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
                    ));
                }
            });

            root.spawn((
                DevSpawnHintText,
                DevPanelUi,
                Text::new("Click terrain to spawn (Shift+select still works)"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.65, 0.75, 0.85, 1.0)),
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
                Node {
                    display: Display::None,
                    ..default()
                },
            ));

            root.spawn((
                DevDebugText,
                DevPanelUi,
                Text::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.8, 0.85, 0.92, 1.0)),
                Node {
                    display: Display::None,
                    ..default()
                },
            ));

            root.spawn((
                DevWorldToolsText,
                DevPanelUi,
                Text::new("World tools — terrain/scenario utilities (sim controls are above)"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.7, 0.75, 0.82, 1.0)),
                Node {
                    display: Display::None,
                    ..default()
                },
            ));

            super::time_of_day_panel::spawn_time_of_day_section(root);
            super::lighting_panel::spawn_lighting_section(root);

            spawn_debug_toggle_row(root, "Master overlay", DevToggleFlag::Master);
            spawn_debug_toggle_row(root, "Paths", DevToggleFlag::Paths);
            spawn_debug_toggle_row(root, "Steering", DevToggleFlag::Steering);
            spawn_debug_toggle_row(root, "Formations", DevToggleFlag::Formations);
            spawn_debug_toggle_row(root, "Selection gizmos", DevToggleFlag::Selection);
            spawn_debug_toggle_row(root, "Interaction hits", DevToggleFlag::Interaction);
            spawn_debug_toggle_row(root, "Combat overlay", DevToggleFlag::Combat);
            spawn_debug_toggle_row(root, "Health bars (all)", DevToggleFlag::Health);
            spawn_debug_toggle_row(root, "Command trace", DevToggleFlag::CommandTrace);
            spawn_debug_toggle_row(root, "Grid overlay", DevToggleFlag::Grid);
            spawn_debug_toggle_row(root, "Enabled only (E)", DevToggleFlag::EnabledOnly);
            spawn_debug_toggle_row(root, "Reset dev state", DevToggleFlag::ResetDevState);

            super::inspector::setup_inspector_panel(root);
        });
}

fn spawn_debug_toggle_row(parent: &mut ChildSpawnerCommands<'_>, label: &str, flag: DevToggleFlag) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            DevDebugToggleRow,
        ))
        .with_children(|row| {
            row.spawn((
                DevToggleButton { flag },
                DevPanelUi,
                Button,
                Node {
                    width: Val::Px(18.0),
                    height: Val::Px(18.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.55, 0.35, 0.95)),
            ));
            row.spawn((
                DevPanelUi,
                Text::new(label),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.78, 0.84, 0.9, 1.0)),
            ));
        });
}

#[derive(Component, Debug)]
pub(crate) struct DevDebugToggleRow;

fn tab_label(tab: DevTab) -> &'static str {
    match tab {
        DevTab::Units => "Units",
        DevTab::Doodads => "Doodads",
        DevTab::Buildings => "Buildings",
        DevTab::Placement => "Placement",
        DevTab::Scenes => "Scenes",
        DevTab::Inspector => "Inspect",
        DevTab::Debug => "Debug",
        DevTab::WorldTools => "World",
    }
}

/// Show/hide panel when dev mode toggles.
pub(crate) fn sync_dev_panel_visibility(
    dev_state: Res<DevModeState>,
    mut roots: Query<&mut Visibility, With<DevPanelRoot>>,
) {
    if !dev_state.is_changed() {
        return;
    }
    for mut visibility in &mut roots {
        *visibility = if dev_state.enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Refresh list/search/selection text from catalogs.
pub(crate) fn sync_dev_panel_content(
    dev_state: Res<DevModeState>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    building_catalog: Res<BuildingCatalog>,
    scene_registry: Res<DevSceneRegistry>,
    browse_index: Res<CatalogBrowseIndex>,
    mut filter_cache: ResMut<CatalogFilterCache>,
    debounce: Res<DevSearchDebounce>,
    mut texts: ParamSet<(
        Query<&mut Text, (With<DevSearchText>, Without<DevListText>)>,
        Query<
            &mut Text,
            (
                With<DevListText>,
                Without<DevSearchText>,
                Without<DevToolStatusText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevToolStatusText>,
                Without<DevSearchText>,
                Without<DevListText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevSpawnHintText>,
                Without<DevSearchText>,
                Without<DevToolStatusText>,
            ),
        >,
        Query<
            (&DevListRow, &Interaction, &mut Text, &mut BackgroundColor),
            (
                With<DevListRow>,
                Without<DevSearchText>,
                Without<DevListText>,
            ),
        >,
    )>,
) {
    if !dev_state.enabled {
        return;
    }

    if let Ok(mut text) = texts.p0().single_mut() {
        **text = format_search_field_display(&dev_state);
    }

    let catalog_entries = browse_catalog_entries(
        &browse_index,
        &mut filter_cache,
        &unit_catalog,
        &doodad_catalog,
        &building_catalog,
        dev_state.active_tab,
        dev_state.spawn_mode,
        &debounce.filtered_query,
        dev_state.enabled_only,
        &dev_state.favorites,
    )
    .to_vec();

    let scene_entries: Vec<_> = if dev_state.active_tab == DevTab::Scenes {
        scene_registry
            .registry
            .search(&dev_state.scene_name_input)
            .into_iter()
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    if let Ok(mut text) = texts.p1().single_mut() {
        **text = match dev_state.active_tab {
            DevTab::Units | DevTab::Doodads | DevTab::Buildings => {
                format!(
                    "Definitions ({}) — enabled-only: {} — E toggles",
                    catalog_entries.len(),
                    dev_state.enabled_only,
                )
            }
            DevTab::Placement => {
                "Placement tools — select definition on Units/Doodads tab".to_string()
            }
            DevTab::Scenes => format!(
                "Scenes ({}) — click row to load, type to filter/name",
                scene_entries.len()
            ),
            DevTab::Inspector => {
                "World inspector — Alt+click unit, terrain click probes interaction".into()
            }
            DevTab::Debug => "Debug overlay toggles".to_string(),
            DevTab::WorldTools => "World authoring tools".to_string(),
        };
    }

    let show_catalog = matches!(
        dev_state.active_tab,
        DevTab::Units | DevTab::Doodads | DevTab::Buildings
    );
    let show_placement = dev_state.active_tab == DevTab::Placement;
    let show_scenes = dev_state.active_tab == DevTab::Scenes;

    let visible_catalog: Vec<_> = if show_catalog {
        catalog_entries
            .into_iter()
            .skip(dev_state.list_scroll)
            .take(MAX_VISIBLE_ROWS)
            .collect()
    } else {
        Vec::new()
    };

    let visible_scenes: Vec<_> = if show_scenes {
        scene_entries
            .into_iter()
            .skip(dev_state.scene_list_scroll)
            .take(MAX_VISIBLE_ROWS)
            .collect()
    } else {
        Vec::new()
    };

    for (row, interaction, mut text, mut bg) in texts.p4().iter_mut() {
        if show_scenes {
            if row.index < visible_scenes.len() {
                let entry = &visible_scenes[row.index];
                **text = format!("{}  [{}]", entry.name, entry.scene_id);
                let selected =
                    dev_state.selected_scene_id.as_deref() == Some(entry.scene_id.as_str());
                *bg = if selected {
                    BackgroundColor(BTN_BG_ACTIVE)
                } else {
                    menu_button_bg(interaction, false)
                };
            } else {
                **text = String::new();
                *bg = BackgroundColor(Color::srgba(0.08, 0.1, 0.12, 0.5));
            }
            continue;
        }

        if row.index < visible_catalog.len() {
            let entry = &visible_catalog[row.index];
            **text = format_list_row(entry, dev_state.favorites.contains(&entry.definition));
            let selected = dev_state
                .selected_definition
                .as_ref()
                .is_some_and(|sel| sel == &entry.definition);
            *bg = if selected {
                BackgroundColor(BTN_BG_ACTIVE)
            } else {
                menu_button_bg(interaction, false)
            };
        } else {
            **text = String::new();
            *bg = BackgroundColor(Color::srgba(0.08, 0.1, 0.12, 0.5));
        }
    }

    if let Ok(mut text) = texts.p2().single_mut() {
        **text = dev_state.tool_status_text();
    }

    if let Ok(mut text) = texts.p3().single_mut() {
        **text = if dev_state.active_tab == DevTab::Scenes {
            if dev_state.last_scene_message.is_empty() {
                "Scenes tab: type name, Save Current World, click row to load".into()
            } else {
                dev_state.last_scene_message.clone()
            }
        } else if dev_state.last_spawn_message.is_empty() {
            if show_placement {
                "Left-click terrain to place · Esc or right-click cancels".into()
            } else {
                "Select a definition · T=cycle team · 1-9 favorites · Ctrl+1-9 assign".into()
            }
        } else {
            dev_state.last_spawn_message.clone()
        };
    }
}

/// Tab-specific section visibility (Node queries only).
pub(crate) fn sync_dev_panel_section_visibility(
    dev_state: Res<DevModeState>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<DevDebugToggleRow>>,
        Query<
            &mut Node,
            (
                With<DevCatalogSection>,
                Without<DevPlacementSection>,
                Without<DevScenesSection>,
            ),
        >,
        Query<
            &mut Node,
            (
                With<DevPlacementSection>,
                Without<DevCatalogSection>,
                Without<DevScenesSection>,
            ),
        >,
        Query<
            &mut Node,
            (
                With<DevScenesSection>,
                Without<DevCatalogSection>,
                Without<DevPlacementSection>,
            ),
        >,
    )>,
) {
    if !dev_state.enabled {
        return;
    }

    let show_catalog = matches!(
        dev_state.active_tab,
        DevTab::Units | DevTab::Doodads | DevTab::Buildings
    );
    let show_placement = dev_state.active_tab == DevTab::Placement;
    let show_scenes = dev_state.active_tab == DevTab::Scenes;
    let show_debug = dev_state.active_tab == DevTab::Debug;

    for mut node in nodes.p0().iter_mut() {
        node.display = if show_debug {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = nodes.p1().single_mut() {
        node.display = if show_catalog {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = nodes.p2().single_mut() {
        node.display = if show_placement {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = nodes.p3().single_mut() {
        node.display = if show_scenes {
            Display::Flex
        } else {
            Display::None
        };
    }
}

/// Tab-specific labels and debug/world summary text.
pub(crate) fn sync_dev_panel_tab_sections(
    dev_state: Res<DevModeState>,
    mut queries: ParamSet<(
        Query<
            (&mut Text, &mut Node),
            (
                With<DevDebugText>,
                Without<DevWorldToolsText>,
                Without<DevSearchText>,
            ),
        >,
        Query<
            (&mut Text, &mut Node),
            (
                With<DevWorldToolsText>,
                Without<DevDebugText>,
                Without<DevSearchText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevPlacementText>,
                Without<DevSearchText>,
                Without<DevListText>,
                Without<DevScenesText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevScenesText>,
                Without<DevSearchText>,
                Without<DevListText>,
                Without<DevPlacementText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevSceneNameText>,
                Without<DevSearchText>,
                Without<DevListText>,
            ),
        >,
    )>,
) {
    if !dev_state.enabled {
        return;
    }

    let show_debug = dev_state.active_tab == DevTab::Debug;
    let show_world = dev_state.active_tab == DevTab::WorldTools;

    if let Ok(mut text) = queries.p3().single_mut() {
        **text = if dev_state.last_scene_message.is_empty() {
            "Save/load WorldData snapshots (not ECS)".into()
        } else {
            dev_state.last_scene_message.clone()
        };
    }

    if let Ok(mut text) = queries.p4().single_mut() {
        **text = format!("Name: {}", dev_state.scene_name_input);
    }

    if let Ok(mut text) = queries.p2().single_mut() {
        let brush = &dev_state.brush;
        **text = format!(
            "Brush: {}  count={}  spacing={:.1}  radius={:.1}\nGrid {}x{}  snap={}  preview={}",
            brush.mode.label(),
            brush.count.min(MAX_BRUSH_SPAWN_COUNT),
            brush.spacing,
            brush.scatter_radius,
            brush.grid_columns,
            brush.grid_rows,
            dev_state.terrain_conforming,
            dev_state.show_preview,
        );
    }

    if let Ok((mut text, mut node)) = queries.p0().single_mut() {
        node.display = if show_debug {
            Display::Flex
        } else {
            Display::None
        };
        **text = format_debug_summary(&dev_state.debug_config);
    }

    if let Ok((mut text, mut node)) = queries.p1().single_mut() {
        node.display = if show_world {
            Display::Flex
        } else {
            Display::None
        };
        **text = if dev_state.pile_harness_message.is_empty()
            && dev_state.treasury_harness_message.is_empty()
        {
            "World tools — piles: P/D/O/H/G/L/V · treasuries: C/Y/E/B/J (see ADR-090, ADR-093)"
                .into()
        } else {
            format!(
                "{}\n{}",
                if dev_state.pile_harness_message.is_empty() {
                    "Piles: (no status)".to_string()
                } else {
                    dev_state.pile_harness_message.clone()
                },
                if dev_state.treasury_harness_message.is_empty() {
                    "Treasury: (no status)".to_string()
                } else {
                    dev_state.treasury_harness_message.clone()
                }
            )
        };
    }
}

/// Simulation pause/tick readout (issues requests only; does not own control state).
pub(crate) fn sync_dev_simulation_status(
    dev_state: Res<DevModeState>,
    control: Res<SimulationControlState>,
    mut text: Query<&mut Text, With<DevSimulationStatus>>,
) {
    if !dev_state.enabled {
        return;
    }
    let Ok(mut label) = text.single_mut() else {
        return;
    };
    let state = if control.paused {
        if control.step_once {
            "stepping"
        } else {
            "paused"
        }
    } else {
        "running"
    };
    **label = format!(
        "Sim: {state:<8} tick {tick:>6}  Space pause · Shift+Space step",
        state = state,
        tick = control.current_tick,
    );
}

/// Fixed-size menu buttons: hover/pressed/active visuals without layout shift.
pub(crate) fn sync_dev_panel_button_styles(
    dev_state: Res<DevModeState>,
    mut tabs: Query<(&Interaction, &DevTabButton, &mut BackgroundColor), With<Button>>,
    mut sim_buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            With<DevSimulationButton>,
            With<Button>,
            Without<DevTabButton>,
        ),
    >,
    mut placement_buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            With<DevPlacementButton>,
            With<Button>,
            Without<DevTabButton>,
            Without<DevSimulationButton>,
        ),
    >,
    mut scene_buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            With<DevSceneButton>,
            With<Button>,
            Without<DevTabButton>,
            Without<DevSimulationButton>,
            Without<DevPlacementButton>,
        ),
    >,
    mut time_buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            With<super::time_of_day_panel::DevTimeOfDayButton>,
            With<Button>,
            Without<DevTabButton>,
            Without<DevSimulationButton>,
            Without<DevPlacementButton>,
            Without<DevSceneButton>,
        ),
    >,
    mut lighting_buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            With<super::lighting_panel::DevLightingTuneButton>,
            With<Button>,
            Without<DevTabButton>,
            Without<DevSimulationButton>,
            Without<DevPlacementButton>,
            Without<DevSceneButton>,
            Without<super::time_of_day_panel::DevTimeOfDayButton>,
        ),
    >,
    mut toggle_buttons: Query<
        (&Interaction, &DevToggleButton, &mut BackgroundColor),
        (
            With<Button>,
            Without<DevTabButton>,
            Without<DevSimulationButton>,
            Without<DevPlacementButton>,
            Without<DevSceneButton>,
            Without<super::time_of_day_panel::DevTimeOfDayButton>,
            Without<super::lighting_panel::DevLightingTuneButton>,
        ),
    >,
) {
    if !dev_state.enabled {
        return;
    }

    let flags = dev_state.debug_config;

    for (interaction, tab_button, mut bg) in &mut tabs {
        *bg = menu_button_bg(interaction, dev_state.active_tab == tab_button.tab);
    }

    for (interaction, mut bg) in &mut sim_buttons {
        *bg = menu_button_bg(interaction, false);
    }

    for (interaction, mut bg) in &mut placement_buttons {
        *bg = menu_button_bg(interaction, false);
    }

    for (interaction, mut bg) in &mut scene_buttons {
        *bg = menu_button_bg(interaction, false);
    }

    for (interaction, mut bg) in &mut time_buttons {
        *bg = menu_button_bg(interaction, false);
    }

    for (interaction, mut bg) in &mut lighting_buttons {
        *bg = menu_button_bg(interaction, false);
    }

    for (interaction, toggle, mut bg) in &mut toggle_buttons {
        let on = match toggle.flag {
            DevToggleFlag::Master => flags.enabled,
            DevToggleFlag::Paths => flags.path,
            DevToggleFlag::Steering => flags.steering,
            DevToggleFlag::Formations => flags.formation,
            DevToggleFlag::Selection => flags.selection,
            DevToggleFlag::Interaction => flags.interaction,
            DevToggleFlag::Combat => flags.combat,
            DevToggleFlag::Health => flags.health,
            DevToggleFlag::CommandTrace => flags.intent,
            DevToggleFlag::Grid => flags.grid,
            DevToggleFlag::EnabledOnly => dev_state.enabled_only,
            DevToggleFlag::ResetDevState => false,
        };
        *bg = menu_button_bg(interaction, on);
    }
}

fn format_list_row(entry: &CatalogBrowserEntry, favorite: bool) -> String {
    let star = if favorite { "★ " } else { "  " };
    let label = truncate_label(&entry.label, MAX_LIST_LABEL_CHARS.saturating_sub(12));
    let id_or_key = if entry.render_key.is_empty() {
        entry.definition.id_str()
    } else {
        entry.render_key.as_str()
    };
    let id_or_key = truncate_label(id_or_key, 18);
    format!("{star}{label}  [{}]  {id_or_key}", entry.category)
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    }
}

fn format_search_field_display(dev_state: &DevModeState) -> String {
    use super::dev_mode::DevTextFieldFocus;

    if dev_state.active_tab == DevTab::Scenes {
        let focused = dev_state.text_focus == DevTextFieldFocus::SceneName;
        if dev_state.scene_name_input.is_empty() && !focused {
            return "Scene name… (click or type)".to_string();
        }
        return dev_state.scene_name_input.clone();
    }

    let focused = dev_state.text_focus == DevTextFieldFocus::CatalogSearch;
    if dev_state.search_query.is_empty() && !focused {
        return "Search definitions… (Ctrl+F or /)".to_string();
    }
    dev_state.search_query.clone()
}

/// Search box focus border/background (DV2).
pub(crate) fn sync_dev_search_box_style(
    dev_state: Res<DevModeState>,
    mut boxes: Query<(&mut BackgroundColor, &mut BorderColor), With<DevSearchBox>>,
    mut search_text: Query<&mut TextColor, With<DevSearchText>>,
    mut clear_buttons: Query<&mut Visibility, With<DevSearchClearButton>>,
) {
    if !dev_state.enabled {
        return;
    }

    let focused = dev_state.has_text_focus();
    for (mut bg, mut border) in &mut boxes {
        *bg = BackgroundColor(if focused {
            SEARCH_BG_FOCUSED
        } else {
            SEARCH_BG_IDLE
        });
        border.set_all(if focused {
            SEARCH_BORDER_FOCUSED
        } else {
            SEARCH_BORDER_IDLE
        });
    }

    if let Ok(mut color) = search_text.single_mut() {
        *color = TextColor(if focused {
            Color::srgba(0.92, 0.95, 0.98, 1.0)
        } else if dev_state.search_query.is_empty() && dev_state.active_tab != DevTab::Scenes {
            Color::srgba(0.65, 0.72, 0.80, 1.0)
        } else {
            Color::srgba(0.85, 0.90, 0.95, 1.0)
        });
    }

    let show_clear = match dev_state.active_tab {
        DevTab::Scenes => {
            !dev_state.scene_name_input.is_empty()
                && dev_state.text_focus == super::dev_mode::DevTextFieldFocus::SceneName
        }
        _ => {
            !dev_state.search_query.is_empty()
                && dev_state.text_focus == super::dev_mode::DevTextFieldFocus::CatalogSearch
        }
    };
    for mut visibility in &mut clear_buttons {
        *visibility = if show_clear {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn format_debug_summary(flags: &DevDebugFlags) -> String {
    format!(
        "Overlay master: {}\nPaths: {}  Steering: {}  Formations: {}\nSelection: {}  Interaction: {}  Combat: {}  Health: {}  Trace: {}  Grid: {}",
        flags.enabled,
        flags.path,
        flags.steering,
        flags.formation,
        flags.selection,
        flags.interaction,
        flags.combat,
        flags.health,
        flags.intent,
        flags.grid,
    )
}

/// Handle tab, list, and debug toggle button presses.
pub(crate) fn handle_dev_panel_ui_interaction(
    mut dev_state: ResMut<DevModeState>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    building_catalog: Res<BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    interior_catalog: Res<InteriorProfileCatalog>,
    browse_index: Res<CatalogBrowseIndex>,
    mut filter_cache: ResMut<CatalogFilterCache>,
    mut debounce: ResMut<DevSearchDebounce>,
    mut world: ResMut<WorldData>,
    mut scene_registry: ResMut<DevSceneRegistry>,
    runtime: Option<Res<DoodadsRuntimeSettings>>,
    camera_state: Query<&RtsCameraState, With<RtsCamera>>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut sim_requests: ResMut<SimulationControlRequests>,
    mut buttons: ParamSet<(
        Query<(&Interaction, &DevTabButton), Changed<Interaction>>,
        Query<(&Interaction, &DevListRow), Changed<Interaction>>,
        Query<(&Interaction, &DevToggleButton), Changed<Interaction>>,
        Query<(&Interaction, &DevPlacementButton), Changed<Interaction>>,
        Query<(&Interaction, &DevSceneButton), Changed<Interaction>>,
        Query<(&Interaction, &DevSimulationButton), Changed<Interaction>>,
        Query<&Interaction, (With<DevSearchBox>, Changed<Interaction>)>,
        Query<&Interaction, (With<DevSearchClearButton>, Changed<Interaction>)>,
    )>,
) {
    if !dev_state.enabled {
        return;
    }

    for interaction in buttons.p6().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            if dev_state.active_tab == DevTab::Scenes {
                dev_state.focus_scene_name();
            } else {
                dev_state.focus_catalog_search();
            }
        }
    }

    for interaction in buttons.p7().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            if dev_state.active_tab == DevTab::Scenes {
                dev_state.scene_name_input.clear();
                dev_state.scene_list_scroll = 0;
                dev_state.focus_scene_name();
            } else {
                dev_state.search_query.clear();
                dev_state.list_scroll = 0;
                debounce.note_input(&dev_state.search_query);
                dev_state.focus_catalog_search();
            }
        }
    }

    let mut panel_click_without_search = false;

    for (interaction, button) in buttons.p5().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click_without_search = true;
        match button.action {
            DevSimulationAction::TogglePause => sim_requests.toggle_pause = true,
            DevSimulationAction::StepOnce => sim_requests.step_once = true,
        }
    }

    for (interaction, button) in buttons.p0().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            panel_click_without_search = true;
            dev_state.active_tab = button.tab;
            dev_state.list_scroll = 0;
            dev_state.scene_list_scroll = 0;
        }
    }

    let active_tab = dev_state.active_tab;
    let search_query = debounce.filtered_query.clone();
    let scene_name_input = dev_state.scene_name_input.clone();
    let list_scroll = dev_state.list_scroll;
    let scene_list_scroll = dev_state.scene_list_scroll;
    let spawn_mode = dev_state.spawn_mode;
    let enabled_only = dev_state.enabled_only;

    let entries = browse_catalog_entries(
        &browse_index,
        &mut filter_cache,
        &unit_catalog,
        &doodad_catalog,
        &building_catalog,
        active_tab,
        spawn_mode,
        &search_query,
        enabled_only,
        &dev_state.favorites,
    )
    .to_vec();

    let scene_entries: Vec<_> = if active_tab == DevTab::Scenes {
        scene_registry
            .registry
            .search(&scene_name_input)
            .into_iter()
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    for (interaction, row) in buttons.p1().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click_without_search = true;
        if active_tab == DevTab::Scenes {
            let index = scene_list_scroll + row.index;
            if let Some(entry) = scene_entries.get(index) {
                dev_state.selected_scene_id = Some(entry.scene_id.clone());
                match load_scene_by_id(
                    &mut world,
                    &unit_catalog,
                    &doodad_catalog,
                    &building_catalog,
                    &footprint_catalog,
                    &interior_catalog,
                    &scene_registry.registry,
                    &entry.scene_id,
                ) {
                    Ok(report) => {
                        dev_state.last_loaded_scene_id = Some(entry.scene_id.clone());
                        dev_state.last_scene_message = format!(
                            "Loaded {} — units={} doodads={} seed={} ({}ms)",
                            entry.name,
                            report.units_loaded,
                            report.doodads_loaded,
                            report.world_seed,
                            report.elapsed_ms
                        );
                    }
                    Err(err) => {
                        dev_state.last_scene_message = format!("Load failed: {err}");
                    }
                }
            }
            continue;
        }
        let index = list_scroll + row.index;
        if let Some(entry) = entries.get(index) {
            dev_state.select_definition(entry.definition.clone());
        }
    }

    for (interaction, button) in buttons.p2().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click_without_search = true;
        toggle_dev_flag(&mut dev_state, button.flag, &mut debounce);
    }

    for (interaction, button) in buttons.p3().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click_without_search = true;
        apply_placement_action(&mut dev_state, button.action);
    }

    for (interaction, button) in buttons.p4().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click_without_search = true;
        apply_scene_action(
            button.action,
            &mut dev_state,
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &interior_catalog,
            &mut scene_registry,
            runtime.as_deref(),
            camera_state.iter().next(),
        );
    }

    if panel_click_without_search {
        dev_state.clear_text_focus();
    }
}

fn apply_placement_action(state: &mut DevModeState, action: DevPlacementAction) {
    match action {
        DevPlacementAction::CycleBrush => {
            state.brush.mode = state.brush.mode.next();
        }
        DevPlacementAction::CountUp => {
            state.brush.count = (state.brush.count + 1).min(MAX_BRUSH_SPAWN_COUNT);
        }
        DevPlacementAction::CountDown => {
            state.brush.count = state.brush.count.saturating_sub(1).max(1);
        }
        DevPlacementAction::SpacingUp => {
            state.brush.spacing = (state.brush.spacing + 0.5).min(64.0);
        }
        DevPlacementAction::SpacingDown => {
            state.brush.spacing = (state.brush.spacing - 0.5).max(0.5);
        }
        DevPlacementAction::RadiusUp => {
            state.brush.scatter_radius = (state.brush.scatter_radius + 1.0).min(128.0);
        }
        DevPlacementAction::RadiusDown => {
            state.brush.scatter_radius = (state.brush.scatter_radius - 1.0).max(1.0);
        }
        DevPlacementAction::ToggleTerrainSnap => {
            state.terrain_conforming = !state.terrain_conforming;
        }
        DevPlacementAction::TogglePreview => {
            state.show_preview = !state.show_preview;
        }
        DevPlacementAction::CycleSpawnTeam => {
            state.cycle_spawn_affiliation();
        }
    }
}

fn apply_scene_action(
    action: DevSceneAction,
    dev_state: &mut DevModeState,
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    scene_registry: &mut DevSceneRegistry,
    runtime: Option<&DoodadsRuntimeSettings>,
    camera: Option<&RtsCameraState>,
) {
    let world_seed = runtime
        .map(|settings| settings.world_seed)
        .unwrap_or(crate::doodads::DEFAULT_DOODAD_WORLD_SEED);
    let debug_flags = Some(SceneDebugFlagsSnapshot::from(dev_state.debug_config));

    match action {
        DevSceneAction::SaveCurrent => {
            let name = if dev_state.scene_name_input.trim().is_empty() {
                "Untitled Scene".to_string()
            } else {
                dev_state.scene_name_input.clone()
            };
            match save_current_world(
                world,
                &mut scene_registry.registry,
                &name,
                world_seed,
                debug_flags,
                camera,
            ) {
                Ok(scene_id) => {
                    dev_state.selected_scene_id = Some(scene_id.clone());
                    dev_state.last_loaded_scene_id = Some(scene_id.clone());
                    dev_state.last_scene_message = format!("Saved scene '{name}' as {scene_id}");
                }
                Err(err) => dev_state.last_scene_message = format!("Save failed: {err}"),
            }
        }
        DevSceneAction::ReloadLast => {
            let Some(scene_id) = dev_state.last_loaded_scene_id.clone() else {
                dev_state.last_scene_message = "No scene loaded yet".into();
                return;
            };
            match load_scene_by_id(
                world,
                unit_catalog,
                doodad_catalog,
                building_catalog,
                footprint_catalog,
                interior_catalog,
                &scene_registry.registry,
                &scene_id,
            ) {
                Ok(report) => {
                    dev_state.last_scene_message = format!(
                        "Reloaded {scene_id} — units={} doodads={} ({}ms)",
                        report.units_loaded, report.doodads_loaded, report.elapsed_ms
                    );
                }
                Err(err) => dev_state.last_scene_message = format!("Reload failed: {err}"),
            }
        }
        DevSceneAction::ClearWorld => {
            clear_dev_world(world);
            dev_state.last_scene_message = "Cleared all units and doodads".into();
        }
        DevSceneAction::DeleteSelected => {
            let Some(scene_id) = dev_state.selected_scene_id.clone() else {
                dev_state.last_scene_message = "Select a scene row first".into();
                return;
            };
            match delete_scene(&mut scene_registry.registry, &scene_id) {
                Ok(()) => {
                    if dev_state.last_loaded_scene_id.as_deref() == Some(scene_id.as_str()) {
                        dev_state.last_loaded_scene_id = None;
                    }
                    dev_state.selected_scene_id = None;
                    dev_state.last_scene_message = format!("Deleted scene {scene_id}");
                }
                Err(err) => dev_state.last_scene_message = format!("Delete failed: {err}"),
            }
        }
    }
}

fn toggle_dev_flag(
    state: &mut DevModeState,
    flag: DevToggleFlag,
    debounce: &mut DevSearchDebounce,
) {
    match flag {
        DevToggleFlag::EnabledOnly => state.enabled_only = !state.enabled_only,
        DevToggleFlag::Master => state.debug_config.enabled = !state.debug_config.enabled,
        DevToggleFlag::Paths => state.debug_config.path = !state.debug_config.path,
        DevToggleFlag::Steering => state.debug_config.steering = !state.debug_config.steering,
        DevToggleFlag::Formations => state.debug_config.formation = !state.debug_config.formation,
        DevToggleFlag::Selection => state.debug_config.selection = !state.debug_config.selection,
        DevToggleFlag::Interaction => {
            state.debug_config.interaction = !state.debug_config.interaction
        }
        DevToggleFlag::Combat => state.debug_config.combat = !state.debug_config.combat,
        DevToggleFlag::Health => state.debug_config.health = !state.debug_config.health,
        DevToggleFlag::CommandTrace => state.debug_config.intent = !state.debug_config.intent,
        DevToggleFlag::Grid => state.debug_config.grid = !state.debug_config.grid,
        DevToggleFlag::ResetDevState => {
            let enabled = state.enabled;
            let active_tab = state.active_tab;
            state.reset_tool_state();
            state.enabled = enabled;
            state.active_tab = active_tab;
            debounce.force_sync("");
        }
    }
}
