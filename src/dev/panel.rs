//! Dev mode panel UI (Bevy UI, ADR-043).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::world::{BuildingCatalog, BuildingCatalogRevision, DoodadCatalog, UnitCatalog};

use super::catalog::{
    DevCatalogStatusText, DevContextualPlacementAction, DevContextualPlacementButton,
    DevContextualPlacementSection, DevContextualPlacementTitle, DevPlacementActiveBanner,
    DevTabChrome, all_catalog_tabs, spawn_tab_label,
};
use super::catalog_browser::CatalogBrowserEntry;
use super::catalog_cache::{
    CatalogBrowseIndex, CatalogFilterCache, DevSearchDebounce, browse_catalog_entries,
};
use super::dev_mode::{DevModeState, DevTab, ItemsBrowserSubtab};
use super::input::{DevPanelRoot, DevPanelUi};
use super::tools::MAX_BRUSH_SPAWN_COUNT;
use super::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::widgets::{
    CATALOG_SEARCH_PLACEHOLDER, CATALOG_SEARCH_TOOLTIP, FIELD_BG_FOCUSED, FIELD_BG_IDLE,
    FIELD_BORDER_FOCUSED, FIELD_BORDER_IDLE,
};

use crate::simulation::{SimulationControlRequests, SimulationControlState};

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
const SEARCH_BG_IDLE: Color = FIELD_BG_IDLE;
const SEARCH_BG_FOCUSED: Color = FIELD_BG_FOCUSED;
const SEARCH_BORDER_IDLE: Color = FIELD_BORDER_IDLE;
const SEARCH_BORDER_FOCUSED: Color = FIELD_BORDER_FOCUSED;

#[derive(SystemParam)]
pub(crate) struct DevPanelCatalogResources<'w> {
    unit_catalog: Res<'w, UnitCatalog>,
    doodad_catalog: Res<'w, DoodadCatalog>,
    building_catalog: Res<'w, BuildingCatalog>,
    building_revision: Res<'w, BuildingCatalogRevision>,
    item_catalog: Res<'w, crate::world::ItemCatalog>,
    item_categories: Res<'w, crate::world::ItemCategoryCatalog>,
    inventory_profiles: Res<'w, crate::world::InventoryProfileCatalog>,
    browse_index: Res<'w, CatalogBrowseIndex>,
}

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
pub(crate) struct DevAssetSizingText;

#[derive(Component, Debug)]
pub(crate) struct DevListText;

#[derive(Component, Debug)]
pub(crate) struct DevSpawnHintText;

#[derive(Component, Debug)]
pub(crate) struct DevCatalogSection;

#[derive(Component, Debug)]
pub(crate) struct DevCatalogTabRow;

#[derive(Component, Debug)]
pub(crate) struct DevTabButton {
    tab: DevTab,
}

#[derive(Component, Debug)]
pub(crate) struct DevListRow {
    index: usize,
}

fn contextual_placement_buttons() -> Vec<(
    &'static str,
    DevContextualPlacementAction,
    super::catalog::PlacementControlField,
)> {
    use super::catalog::PlacementControlField;
    vec![
        (
            "Pattern",
            DevContextualPlacementAction::CycleBrush,
            PlacementControlField::Pattern,
        ),
        (
            "Count +",
            DevContextualPlacementAction::CountUp,
            PlacementControlField::Count,
        ),
        (
            "Count −",
            DevContextualPlacementAction::CountDown,
            PlacementControlField::Count,
        ),
        (
            "Spacing +",
            DevContextualPlacementAction::SpacingUp,
            PlacementControlField::Spacing,
        ),
        (
            "Spacing −",
            DevContextualPlacementAction::SpacingDown,
            PlacementControlField::Spacing,
        ),
        (
            "Radius +",
            DevContextualPlacementAction::RadiusUp,
            PlacementControlField::Radius,
        ),
        (
            "Radius −",
            DevContextualPlacementAction::RadiusDown,
            PlacementControlField::Radius,
        ),
        (
            "Cols +",
            DevContextualPlacementAction::GridColsUp,
            PlacementControlField::GridColumns,
        ),
        (
            "Cols −",
            DevContextualPlacementAction::GridColsDown,
            PlacementControlField::GridColumns,
        ),
        (
            "Rows +",
            DevContextualPlacementAction::GridRowsUp,
            PlacementControlField::GridRows,
        ),
        (
            "Rows −",
            DevContextualPlacementAction::GridRowsDown,
            PlacementControlField::GridRows,
        ),
        (
            "Team",
            DevContextualPlacementAction::CycleSpawnTeam,
            PlacementControlField::Affiliation,
        ),
        (
            "Terrain snap",
            DevContextualPlacementAction::ToggleTerrainSnap,
            PlacementControlField::TerrainSnap,
        ),
        (
            "Preview",
            DevContextualPlacementAction::TogglePreview,
            PlacementControlField::Preview,
        ),
        (
            "Yaw +",
            DevContextualPlacementAction::RotationUp,
            PlacementControlField::Rotation,
        ),
        (
            "Yaw −",
            DevContextualPlacementAction::RotationDown,
            PlacementControlField::Rotation,
        ),
        (
            "Scale +",
            DevContextualPlacementAction::ScaleUp,
            PlacementControlField::Scale,
        ),
        (
            "Scale −",
            DevContextualPlacementAction::ScaleDown,
            PlacementControlField::Scale,
        ),
        (
            "Cancel placement",
            DevContextualPlacementAction::CancelPlacement,
            PlacementControlField::Cancel,
        ),
    ]
}

/// Spawn legacy panel content inside the catalog dev window body (Slice 3).
pub(crate) fn setup_dev_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    let Some((body, _)) = bodies
        .iter()
        .find(|(_, body)| body.id == super::window::DevWindowId::Catalog)
    else {
        return;
    };
    commands.entity(body).with_children(|panel| {
        panel
            .spawn((
                DevPanelRoot,
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
                    DevCatalogTabRow,
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    },
                ))
                .with_children(|tabs| {
                    for tab in all_catalog_tabs() {
                        tabs.spawn((
                            DevTabChrome { tab: *tab },
                            DevTabButton { tab: *tab },
                            DevPanelUi,
                            Button,
                            Node {
                                height: Val::Px(MENU_BTN_HEIGHT_PX),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(BTN_BG_IDLE),
                            Text::new(spawn_tab_label(*tab)),
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
                    Text::new(""),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.70, 0.88, 0.78, 1.0)),
                    Node {
                        display: Display::None,
                        ..default()
                    },
                ));

                root.spawn((
                    DevAssetSizingText,
                    DevPanelUi,
                    Text::new(""),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.65, 0.78, 0.88, 1.0)),
                    Node {
                        display: Display::None,
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
                            DevTooltipTarget::new(CATALOG_SEARCH_TOOLTIP),
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
                                Text::new("Search definitions... (Ctrl+F or /)"),
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
                                Text::new("x"),
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

                    catalog.spawn((
                        DevCatalogStatusText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.70, 0.88, 0.78, 1.0)),
                    ));

                    catalog
                        .spawn((
                            DevContextualPlacementSection,
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
                                DevContextualPlacementTitle,
                                DevPanelUi,
                                Text::new("Placement"),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
                            ));
                            for (label, action, field) in contextual_placement_buttons() {
                                placement.spawn((
                                    DevContextualPlacementButton { action, field },
                                    DevTooltipTarget::new(
                                        super::catalog::placement_control_tooltip(field),
                                    ),
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
                });

                root.spawn((
                    DevPlacementActiveBanner,
                    DevPanelUi,
                    Text::new(""),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.85, 0.75, 0.45, 1.0)),
                    Node {
                        min_height: Val::Px(14.0),
                        ..default()
                    },
                ));

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

                super::inventory_tools::panel::spawn_items_section(root);
            });
    });
}

/// Refresh list/search/selection text from catalogs.
pub(crate) fn sync_dev_panel_content(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    building_catalog: Res<BuildingCatalog>,
    building_revision: Res<BuildingCatalogRevision>,
    item_catalog: Res<crate::world::ItemCatalog>,
    item_categories: Res<crate::world::ItemCategoryCatalog>,
    inventory_profiles: Res<crate::world::InventoryProfileCatalog>,
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
                Without<DevAssetSizingText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevAssetSizingText>,
                Without<DevSearchText>,
                Without<DevToolStatusText>,
                Without<DevSpawnHintText>,
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
    if !dev_state.enabled || !registry.is_visible(DevWindowId::Catalog) {
        return;
    }

    if let Ok(mut text) = texts.p0().single_mut() {
        **text = format_search_field_display(&dev_state);
    }

    let catalog_entries: Vec<CatalogBrowserEntry> = if dev_state.active_tab == DevTab::Items {
        super::items_browser::items_catalog_browser_entries(
            &item_catalog,
            &item_categories,
            &inventory_profiles,
            dev_state.inventory.subtab,
            &debounce.filtered_query,
            dev_state.enabled_only,
        )
    } else {
        browse_catalog_entries(
            &browse_index,
            &mut filter_cache,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            building_revision.0,
            dev_state.active_tab,
            dev_state.spawn_mode,
            &debounce.filtered_query,
            dev_state.enabled_only,
            &dev_state.favorites,
        )
        .to_vec()
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
            DevTab::Items => match dev_state.inventory.subtab {
                ItemsBrowserSubtab::InventoryManage => {
                    "Inventory manage — inspect unit/building/pile, use H subtab tools".to_string()
                }
                ItemsBrowserSubtab::Items | ItemsBrowserSubtab::InventoryProfiles => {
                    format!(
                        "Item catalog ({}) — enabled-only: {}",
                        catalog_entries.len(),
                        dev_state.enabled_only,
                    )
                }
            },
            _ => String::new(),
        };
    }

    let visible_catalog: Vec<_> = catalog_entries
        .into_iter()
        .skip(dev_state.list_scroll)
        .take(MAX_VISIBLE_ROWS)
        .collect();

    for (row, interaction, mut text, mut bg) in texts.p5().iter_mut() {
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
        **text = String::new();
    }

    if let Ok(mut text) = texts.p3().single_mut() {
        **text = String::new();
    }

    if let Ok(mut text) = texts.p4().single_mut() {
        **text = if dev_state.last_spawn_message.is_empty() {
            String::new()
        } else {
            dev_state.last_spawn_message.clone()
        };
    }
}

/// Hide catalog chrome when dev mode is off or the catalog window is closed.
pub(crate) fn sync_dev_catalog_panel_visibility(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut queries: ParamSet<(
        Query<(&mut Visibility, &mut Node), With<DevPanelRoot>>,
        Query<(&mut Visibility, &mut Node), With<DevCatalogTabRow>>,
        Query<(&mut Visibility, &mut Node), With<DevCatalogSection>>,
        Query<(&mut Visibility, &mut Node), With<DevTabChrome>>,
    )>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::Catalog);
    for (mut vis, mut node) in queries.p0().iter_mut() {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (mut vis, mut node) in queries.p1().iter_mut() {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (mut vis, mut node) in queries.p2().iter_mut() {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (mut vis, mut node) in queries.p3().iter_mut() {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
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
    mut buttons: ParamSet<(
        Query<(&Interaction, &DevTabButton, &mut BackgroundColor), With<Button>>,
        Query<
            (&Interaction, &mut BackgroundColor),
            (
                With<DevSimulationButton>,
                With<Button>,
                Without<DevTabButton>,
            ),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (
                With<super::catalog::DevContextualPlacementButton>,
                With<Button>,
                Without<DevTabButton>,
                Without<DevSimulationButton>,
            ),
        >,
    )>,
) {
    if !dev_state.enabled {
        return;
    }

    for (interaction, tab_button, mut bg) in buttons.p0().iter_mut() {
        *bg = menu_button_bg(interaction, dev_state.active_tab == tab_button.tab);
    }

    for (interaction, mut bg) in buttons.p1().iter_mut() {
        *bg = menu_button_bg(interaction, false);
    }

    for (interaction, mut bg) in buttons.p2().iter_mut() {
        *bg = menu_button_bg(interaction, false);
    }
}

fn format_list_row(entry: &CatalogBrowserEntry, favorite: bool) -> String {
    let star = if favorite { "[*] " } else { "    " };
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
            "{}...",
            value
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn format_search_field_display(dev_state: &DevModeState) -> String {
    use super::dev_mode::DevTextFieldFocus;

    let focused = dev_state.text_focus == DevTextFieldFocus::CatalogSearch;
    if dev_state.search_query.is_empty() && !focused {
        return CATALOG_SEARCH_PLACEHOLDER.to_string();
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

    let focused = dev_state.text_focus == super::dev_mode::DevTextFieldFocus::CatalogSearch;
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
        } else if dev_state.search_query.is_empty() {
            Color::srgba(0.65, 0.72, 0.80, 1.0)
        } else {
            Color::srgba(0.85, 0.90, 0.95, 1.0)
        });
    }

    let show_clear = !dev_state.search_query.is_empty() && focused;
    for mut visibility in &mut clear_buttons {
        *visibility = if show_clear {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Handle tab, list, and catalog button presses.
pub(crate) fn handle_dev_panel_ui_interaction(
    mut dev_state: ResMut<DevModeState>,
    catalogs: DevPanelCatalogResources,
    mut filter_cache: ResMut<CatalogFilterCache>,
    mut debounce: ResMut<DevSearchDebounce>,
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut sim_requests: ResMut<SimulationControlRequests>,
    mut preview: ResMut<crate::dev::tools::DevPlacementPreview>,
    mut buttons: ParamSet<(
        Query<(&Interaction, &DevTabButton), Changed<Interaction>>,
        Query<(&Interaction, &DevListRow), Changed<Interaction>>,
        Query<(&Interaction, &DevContextualPlacementButton), Changed<Interaction>>,
        Query<(&Interaction, &DevSimulationButton), Changed<Interaction>>,
        Query<&Interaction, (With<DevSearchBox>, Changed<Interaction>)>,
        Query<&Interaction, (With<DevSearchClearButton>, Changed<Interaction>)>,
    )>,
) {
    if !dev_state.enabled || !registry.is_visible(DevWindowId::Catalog) {
        return;
    }

    for interaction in buttons.p4().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.focus_catalog_search();
        }
    }

    for interaction in buttons.p5().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.search_query.clear();
            dev_state.list_scroll = 0;
            debounce.note_input(&dev_state.search_query);
            dev_state.focus_catalog_search();
        }
    }

    let mut panel_click_without_search = false;

    for (interaction, button) in buttons.p3().iter() {
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
        }
    }

    let active_tab = dev_state.active_tab;
    let search_query = debounce.filtered_query.clone();
    let list_scroll = dev_state.list_scroll;
    let spawn_mode = dev_state.spawn_mode;
    let enabled_only = dev_state.enabled_only;

    let entries: Vec<CatalogBrowserEntry> = if active_tab == DevTab::Items {
        super::items_browser::items_catalog_browser_entries(
            &catalogs.item_catalog,
            &catalogs.item_categories,
            &catalogs.inventory_profiles,
            dev_state.inventory.subtab,
            &search_query,
            enabled_only,
        )
    } else {
        browse_catalog_entries(
            &catalogs.browse_index,
            &mut filter_cache,
            &catalogs.unit_catalog,
            &catalogs.doodad_catalog,
            &catalogs.building_catalog,
            catalogs.building_revision.0,
            active_tab,
            spawn_mode,
            &search_query,
            enabled_only,
            &dev_state.favorites,
        )
        .to_vec()
    };

    for (interaction, row) in buttons.p1().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click_without_search = true;
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
        apply_contextual_placement_action(&mut dev_state, button.action, &mut preview);
    }

    if panel_click_without_search {
        dev_state.clear_text_focus();
    }
}

fn apply_contextual_placement_action(
    state: &mut DevModeState,
    action: DevContextualPlacementAction,
    preview: &mut crate::dev::tools::DevPlacementPreview,
) {
    match action {
        DevContextualPlacementAction::CycleBrush => {
            state.brush.mode = state.brush.mode.next();
        }
        DevContextualPlacementAction::CountUp => {
            state.brush.count = (state.brush.count + 1).min(MAX_BRUSH_SPAWN_COUNT);
        }
        DevContextualPlacementAction::CountDown => {
            state.brush.count = state.brush.count.saturating_sub(1).max(1);
        }
        DevContextualPlacementAction::SpacingUp => {
            state.brush.spacing = (state.brush.spacing + 0.5).min(64.0);
        }
        DevContextualPlacementAction::SpacingDown => {
            state.brush.spacing = (state.brush.spacing - 0.5).max(0.5);
        }
        DevContextualPlacementAction::RadiusUp => {
            state.brush.scatter_radius = (state.brush.scatter_radius + 1.0).min(128.0);
        }
        DevContextualPlacementAction::RadiusDown => {
            state.brush.scatter_radius = (state.brush.scatter_radius - 1.0).max(1.0);
        }
        DevContextualPlacementAction::GridColsUp => {
            state.brush.grid_columns = (state.brush.grid_columns + 1).min(16);
        }
        DevContextualPlacementAction::GridColsDown => {
            state.brush.grid_columns = state.brush.grid_columns.saturating_sub(1).max(1);
        }
        DevContextualPlacementAction::GridRowsUp => {
            state.brush.grid_rows = (state.brush.grid_rows + 1).min(16);
        }
        DevContextualPlacementAction::GridRowsDown => {
            state.brush.grid_rows = state.brush.grid_rows.saturating_sub(1).max(1);
        }
        DevContextualPlacementAction::ToggleTerrainSnap => {
            state.placement_rules.snap_to_terrain = !state.placement_rules.snap_to_terrain;
            state.terrain_conforming = state.placement_rules.snap_to_terrain;
        }
        DevContextualPlacementAction::TogglePreview => {
            state.show_preview = !state.show_preview;
        }
        DevContextualPlacementAction::CycleSpawnTeam => {
            state.cycle_spawn_affiliation();
        }
        DevContextualPlacementAction::RotationUp => {
            state.placement_yaw_deg = (state.placement_yaw_deg + 5.0) % 360.0;
        }
        DevContextualPlacementAction::RotationDown => {
            state.placement_yaw_deg = (state.placement_yaw_deg - 5.0).rem_euclid(360.0);
        }
        DevContextualPlacementAction::ScaleUp => {
            state.placement_uniform_scale = (state.placement_uniform_scale + 0.05).min(3.0);
        }
        DevContextualPlacementAction::ScaleDown => {
            state.placement_uniform_scale = (state.placement_uniform_scale - 0.05).max(0.1);
        }
        DevContextualPlacementAction::CancelPlacement => {
            super::input::cancel_dev_placement(state, preview);
            state.catalog.set_status("Placement cancelled", 180);
        }
    }
}
