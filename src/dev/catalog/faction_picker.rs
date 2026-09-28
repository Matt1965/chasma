//! Scrollable faction picker for dev unit spawn (replaces cycle-through button).

use bevy::prelude::*;

use super::placement_controls::{PlacementControlSet, placement_control_set, placement_ui_context};
use super::state::CatalogSessionState;
use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::window::DevWindowRegistry;
use crate::world::relationship::{FactionCatalog, FactionDefinition, FactionId};

const ROW_HEIGHT_PX: f32 = 22.0;
const DROPDOWN_MAX_HEIGHT_PX: f32 = 132.0;

#[derive(Component, Debug)]
pub(crate) struct DevSpawnFactionPickerBlock;

#[derive(Component, Debug)]
pub(crate) struct DevSpawnFactionTrigger;

#[derive(Component, Debug)]
pub(crate) struct DevSpawnFactionTriggerLabel;

#[derive(Component, Debug)]
pub(crate) struct DevSpawnFactionDropdown;

#[derive(Component, Debug)]
pub(crate) struct DevSpawnFactionOption {
    pub faction_id: FactionId,
}

pub(crate) fn spawn_spawn_faction_picker(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevSpawnFactionPickerBlock,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|block| {
            block
                .spawn((
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        DevPanelUi,
                        Text::new("Faction"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.72, 0.82, 0.9, 1.0)),
                        Node {
                            min_width: Val::Px(64.0),
                            ..default()
                        },
                    ));
                    row.spawn((
                        DevSpawnFactionTrigger,
                        crate::dev::tooltip::DevTooltipTarget::new(
                            super::placement_control_tooltip(
                                super::placement_controls::PlacementControlField::SpawnFaction,
                            ),
                        ),
                        DevPanelUi,
                        Button,
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.16, 0.22, 0.98)),
                        BorderColor::all(Color::srgba(0.28, 0.42, 0.52, 0.9)),
                    ))
                    .with_children(|trigger| {
                        trigger.spawn((
                            DevSpawnFactionTriggerLabel,
                            DevPanelUi,
                            Text::new("Player"),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.92, 0.96, 0.99, 1.0)),
                        ));
                        trigger.spawn((
                            DevPanelUi,
                            Text::new("v"),
                            TextFont {
                                font_size: 10.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.65, 0.78, 0.88, 1.0)),
                        ));
                    });
                });
            block.spawn((
                DevSpawnFactionDropdown,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(1.0),
                    padding: UiRect::all(Val::Px(2.0)),
                    max_height: Val::Px(DROPDOWN_MAX_HEIGHT_PX),
                    overflow: Overflow::scroll_y(),
                    border: UiRect::all(Val::Px(1.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.12, 0.17, 0.98)),
                BorderColor::all(Color::srgba(0.25, 0.38, 0.48, 0.85)),
            ));
        });
}

fn sorted_enabled_factions(catalog: &FactionCatalog) -> Vec<&FactionDefinition> {
    let mut defs: Vec<_> = catalog.enabled_definitions().collect();
    defs.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    defs
}

fn placement_controls_for_state(
    dev_state: &DevModeState,
    building_catalog: &crate::world::BuildingCatalog,
    doodad_catalog: &crate::world::DoodadCatalog,
) -> PlacementControlSet {
    let ctx = placement_ui_context(dev_state.active_tab, dev_state);
    let building_def = dev_state.selected_definition.as_ref().and_then(|id| {
        if let crate::dev::dev_mode::DefinitionId::Building(bid) = id {
            building_catalog.get(bid)
        } else {
            None
        }
    });
    let doodad_def = dev_state.selected_definition.as_ref().and_then(|id| {
        if let crate::dev::dev_mode::DefinitionId::Doodad(did) = id {
            doodad_catalog.get(did)
        } else {
            None
        }
    });
    placement_control_set(ctx, dev_state.brush.mode, building_def, doodad_def)
}

pub(crate) fn sync_spawn_faction_picker(
    dev_state: Res<DevModeState>,
    faction_catalog: Res<FactionCatalog>,
    building_catalog: Res<crate::world::BuildingCatalog>,
    doodad_catalog: Res<crate::world::DoodadCatalog>,
    registry: Res<DevWindowRegistry>,
    mut block: Query<
        (&mut Visibility, &mut Node),
        (With<DevSpawnFactionPickerBlock>, Without<DevSpawnFactionDropdown>),
    >,
    mut dropdown: Query<
        (&mut Visibility, &mut Node),
        (With<DevSpawnFactionDropdown>, Without<DevSpawnFactionPickerBlock>),
    >,
    mut trigger_label: Query<&mut Text, With<DevSpawnFactionTriggerLabel>>,
    mut option_rows: Query<
        (&DevSpawnFactionOption, &mut BackgroundColor, &mut BorderColor),
    >,
    mut commands: Commands,
    dropdown_entity: Query<Entity, With<DevSpawnFactionDropdown>>,
    existing_options: Query<(Entity, &DevSpawnFactionOption)>,
) {
    let catalog_open =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::Catalog);
    let controls = placement_controls_for_state(&dev_state, &building_catalog, &doodad_catalog);
    let show = catalog_open && controls.spawn_faction;

    if let Ok((mut visibility, mut node)) = block.single_mut() {
        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if show {
            Display::Flex
        } else {
            Display::None
        };
    }

    if !show {
        return;
    }

    let display_name = dev_state
        .spawn_faction_button_label(&faction_catalog)
        .strip_prefix("Faction: ")
        .unwrap_or("Faction")
        .to_string();
    if let Ok(mut text) = trigger_label.single_mut() {
        **text = display_name;
    }

    let open = dev_state.catalog.faction_picker_open;
    if let Ok((mut visibility, mut node)) = dropdown.single_mut() {
        *visibility = if open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if open {
            Display::Flex
        } else {
            Display::None
        };
    }

    let factions = sorted_enabled_factions(&faction_catalog);
    let selected = dev_state.spawn_faction_id.as_str();

    let need_rebuild = existing_options.iter().count() != factions.len()
        || factions.iter().any(|def| {
            !existing_options
                .iter()
                .any(|(_, opt)| opt.faction_id == def.id)
        });

    if need_rebuild {
        for (entity, _) in existing_options.iter() {
            commands.entity(entity).despawn();
        }
        if let Ok(parent) = dropdown_entity.single() {
            for def in factions {
                let is_selected = def.id.as_str() == selected;
                let (bg, border) = option_style(is_selected, false);
                commands.entity(parent).with_children(|list| {
                    list.spawn((
                        DevSpawnFactionOption {
                            faction_id: def.id.clone(),
                        },
                        DevPanelUi,
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(ROW_HEIGHT_PX),
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor::all(border),
                        Text::new(def.display_name.clone()),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.9, 0.95, 0.99, 1.0)),
                    ));
                });
            }
        }
    } else {
        for (option, mut bg, mut border) in option_rows.iter_mut() {
            let is_selected = option.faction_id.as_str() == selected;
            *bg = BackgroundColor(option_style(is_selected, false).0);
            *border = BorderColor::all(option_style(is_selected, false).1);
        }
    }
}

fn option_style(selected: bool, _hovered: bool) -> (Color, Color) {
    if selected {
        (
            Color::srgba(0.18, 0.32, 0.44, 0.98),
            Color::srgba(0.45, 0.62, 0.78, 1.0),
        )
    } else {
        (
            Color::srgba(0.11, 0.17, 0.24, 0.95),
            Color::srgba(0.22, 0.32, 0.4, 0.7),
        )
    }
}

pub(crate) fn handle_spawn_faction_picker(
    mut dev_state: ResMut<DevModeState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    registry: Res<DevWindowRegistry>,
    triggers: Query<&Interaction, (With<DevSpawnFactionTrigger>, Changed<Interaction>)>,
    options: Query<
        (&Interaction, &DevSpawnFactionOption),
        (Changed<Interaction>, With<DevSpawnFactionOption>),
    >,
) {
    if !dev_state.enabled || !registry.is_visible(crate::dev::window::DevWindowId::Catalog) {
        return;
    }

    for interaction in triggers.iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.catalog.faction_picker_open = !dev_state.catalog.faction_picker_open;
        }
    }

    for (interaction, option) in options.iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.select_spawn_faction(option.faction_id.clone());
        }
    }
}

impl CatalogSessionState {
    pub fn close_faction_picker(&mut self) {
        self.faction_picker_open = false;
    }
}
