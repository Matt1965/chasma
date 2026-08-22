//! Dev Items tab UI components (DV0).

use bevy::ecs::system::ParamSet;
use bevy::prelude::*;

use super::input::handle_dev_items_panel_action;
use crate::client::selection::WorldSelectionState;
use crate::dev::dev_mode::{DevModeState, DevTab, DevTextFieldFocus, selected_item_max_stack};
use crate::dev::input::DevPanelUi;
use crate::simulation::SimulationControlState;
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::units::input::SelectedUnits;
use crate::world::{
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemPileSettings, UnitCatalog,
    WorldData,
};

use crate::dev::widgets::theme::{
    BTN_BG_IDLE, FIELD_BG_FOCUSED, FIELD_BG_IDLE, FIELD_BORDER_FOCUSED, FIELD_BORDER_IDLE,
    TEXT_PRIMARY, label_text_font,
};

#[derive(Component, Debug)]
pub struct DevItemsSection;

#[derive(Component, Debug)]
pub struct DevItemsText;

#[derive(Component, Debug)]
pub struct DevItemQuantityRow;

#[derive(Component, Debug)]
pub struct DevItemQuantityBox;

#[derive(Component, Debug)]
pub struct DevItemQuantityText;

#[derive(Component, Debug)]
pub struct DevItemMaxStackText;

#[derive(Component, Debug)]
pub struct DevItemsButton {
    pub action: DevItemsAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevItemsAction {
    QuantityUp,
    QuantityDown,
    QuantityMaxStack,
    CycleEndpoint,
    CycleEntry,
    AddToUnit,
    AddToContainer,
    RemoveEntry,
    ArmPilePlacement,
}

pub fn spawn_items_section(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            DevItemsSection,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|section| {
            section
                .spawn((
                    DevItemQuantityRow,
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                ))
                .with_children(|qty_row| {
                    qty_row
                        .spawn((
                            DevPanelUi,
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(4.0),
                                align_items: AlignItems::Center,
                                flex_wrap: FlexWrap::Wrap,
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                        ))
                        .with_children(|controls| {
                            spawn_qty_button(controls, "−", DevItemsAction::QuantityDown);
                            controls
                                .spawn((
                                    DevItemQuantityBox,
                                    DevPanelUi,
                                    Button,
                                    Node {
                                        min_width: Val::Px(56.0),
                                        min_height: Val::Px(22.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        padding: UiRect::horizontal(Val::Px(6.0)),
                                        border: UiRect::all(Val::Px(1.0)),
                                        ..default()
                                    },
                                    BackgroundColor(FIELD_BG_IDLE),
                                    BorderColor::all(FIELD_BORDER_IDLE),
                                ))
                                .with_children(|field| {
                                    field.spawn((
                                        DevItemQuantityText,
                                        DevPanelUi,
                                        Text::new("10"),
                                        TextFont {
                                            font_size: 12.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgba(0.92, 0.95, 0.98, 1.0)),
                                    ));
                                });
                            spawn_qty_button(controls, "+", DevItemsAction::QuantityUp);
                            spawn_qty_button(controls, "Max", DevItemsAction::QuantityMaxStack);
                            spawn_action_button(controls, "Remove", DevItemsAction::RemoveEntry);
                        });
                    qty_row.spawn((
                        DevItemMaxStackText,
                        DevPanelUi,
                        Text::new("Max stack: —"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.65, 0.78, 0.88, 1.0)),
                    ));
                });

            section.spawn((
                DevItemsText,
                DevPanelUi,
                Text::new("Items / inventory tools"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
            ));

            for (label, action) in [
                ("Target +", DevItemsAction::CycleEndpoint),
                ("Entry +", DevItemsAction::CycleEntry),
                ("Add to Unit", DevItemsAction::AddToUnit),
                ("Add to Container", DevItemsAction::AddToContainer),
                ("Spawn pile", DevItemsAction::ArmPilePlacement),
            ] {
                spawn_action_button(section, label, action);
            }
        });
}

fn spawn_qty_button(parent: &mut ChildSpawnerCommands, label: &str, action: DevItemsAction) {
    parent.spawn((
        DevItemsButton { action },
        DevPanelUi,
        Button,
        Node {
            min_width: Val::Px(24.0),
            min_height: Val::Px(22.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(BTN_BG_IDLE),
        Text::new(label),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
    ));
}

fn spawn_action_button(parent: &mut ChildSpawnerCommands, label: &str, action: DevItemsAction) {
    parent.spawn((
        DevItemsButton { action },
        DevPanelUi,
        Button,
        Interaction::None,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BTN_BG_IDLE),
        Text::new(label),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
    ));
}

pub fn sync_items_section_visibility(
    dev_state: Res<DevModeState>,
    mut section: Query<(&mut Visibility, &mut Node), With<DevItemsSection>>,
) {
    let show = dev_state.enabled && dev_state.active_tab == DevTab::Items;
    for (mut visibility, mut node) in &mut section {
        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if show { Display::Flex } else { Display::None };
    }
}

pub fn sync_item_quantity_controls(
    dev_state: Res<DevModeState>,
    items: Res<ItemCatalog>,
    mut texts: ParamSet<(
        Query<&mut Text, With<DevItemQuantityText>>,
        Query<&mut Text, (With<DevItemMaxStackText>, Without<DevItemQuantityText>)>,
    )>,
    mut qty_boxes: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<DevItemQuantityBox>, Without<DevItemQuantityText>),
    >,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::Items {
        return;
    }

    let focused = dev_state.text_focus == DevTextFieldFocus::ItemQuantity;
    let display_qty = if focused {
        if dev_state.inventory.quantity_input.is_empty() {
            "…".to_string()
        } else {
            dev_state.inventory.quantity_input.clone()
        }
    } else {
        dev_state.inventory.quantity.to_string()
    };

    if let Ok(mut text) = texts.p0().single_mut() {
        **text = display_qty;
    }

    if let Ok(mut text) = texts.p1().single_mut() {
        **text = format_max_stack_label(dev_state.selected_definition.as_ref(), &items);
    }

    for (mut bg, mut border) in &mut qty_boxes {
        *bg = BackgroundColor(if focused {
            FIELD_BG_FOCUSED
        } else {
            FIELD_BG_IDLE
        });
        border.set_all(if focused {
            FIELD_BORDER_FOCUSED
        } else {
            FIELD_BORDER_IDLE
        });
    }
}

fn format_max_stack_label(
    selected: Option<&crate::dev::dev_mode::DefinitionId>,
    items: &ItemCatalog,
) -> String {
    match selected_item_max_stack(selected, items) {
        Some(max) => {
            if let Some(crate::dev::dev_mode::DefinitionId::Item(item_id)) = selected {
                let name = items
                    .get(item_id)
                    .map(|item| item.display_name.as_str())
                    .unwrap_or(item_id.as_str());
                format!("Max stack: {max} ({name})")
            } else {
                format!("Max stack: {max}")
            }
        }
        None => "Max stack: — (select an item)".into(),
    }
}

pub fn sync_items_panel_text(
    dev_state: Res<DevModeState>,
    world: Res<WorldData>,
    world_selection: Res<WorldSelectionState>,
    building_selection: Res<GameplayBuildingSelection>,
    selection: Res<SelectedUnits>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    mut texts: Query<&mut Text, With<DevItemsText>>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::Items {
        return;
    }
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let ctx = crate::world::InventoryCatalogCtx::new(&items, &categories, &profiles);
    **text = super::format::format_inventory_tool_panel(
        &world,
        &world_selection,
        &building_selection,
        &selection,
        &items,
        &categories,
        &ctx,
        world.item_instance_store(),
        &dev_state.inventory,
        dev_state.selected_definition.as_ref(),
    );
}

pub fn handle_dev_items_buttons(
    mut dev_state: ResMut<DevModeState>,
    mut world: ResMut<WorldData>,
    world_selection: Res<WorldSelectionState>,
    building_selection: Res<GameplayBuildingSelection>,
    selection: Res<SelectedUnits>,
    unit_catalog: Res<UnitCatalog>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    pile_settings: Res<ItemPileSettings>,
    simulation: Res<SimulationControlState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut buttons: ParamSet<(
        Query<(&Interaction, &DevItemsButton), (Changed<Interaction>, Without<DevItemQuantityBox>)>,
        Query<
            &Interaction,
            (
                With<DevItemQuantityBox>,
                Changed<Interaction>,
                Without<DevItemsButton>,
            ),
        >,
    )>,
) {
    if !dev_state.enabled {
        return;
    }

    for interaction in buttons.p1().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.focus_item_quantity();
        }
    }

    for (interaction, button) in buttons.p0().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        if dev_state.active_tab != DevTab::Items {
            dev_state.inventory.message =
                "Switch to the Items tab before using inventory tools".into();
            continue;
        }
        dev_state.apply_item_quantity_input();
        match button.action {
            DevItemsAction::QuantityUp => dev_state.bump_item_quantity(1),
            DevItemsAction::QuantityDown => dev_state.bump_item_quantity(-1),
            DevItemsAction::QuantityMaxStack => dev_state.set_item_quantity_to_max_stack(&items),
            other => handle_dev_items_panel_action(
                &mut dev_state,
                &mut world,
                &world_selection,
                &building_selection,
                &selection,
                &unit_catalog,
                &items,
                &categories,
                &profiles,
                &pile_settings,
                &simulation,
                other,
            ),
        }
    }
}
