//! Dev Items tab UI components (DV0).

use bevy::prelude::*;

use super::input::handle_dev_items_panel_action;
use crate::dev::dev_mode::{selected_item_max_stack, DevModeState, DevTab, DevTextFieldFocus};
use crate::dev::input::DevPanelUi;
use crate::dev::inspector::WorldInspectorState;
use crate::simulation::SimulationControlState;
use crate::units::input::SelectedUnits;
use crate::world::{
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemPileSettings, UnitCatalog,
    WorldData,
};

const QTY_FIELD_BG_IDLE: Color = Color::srgba(0.08, 0.11, 0.14, 0.95);
const QTY_FIELD_BG_FOCUSED: Color = Color::srgba(0.10, 0.18, 0.24, 0.98);
const QTY_FIELD_BORDER_IDLE: Color = Color::srgba(0.25, 0.32, 0.38, 0.9);
const QTY_FIELD_BORDER_FOCUSED: Color = Color::srgba(0.35, 0.75, 0.55, 1.0);
const QTY_BTN_BG: Color = Color::srgba(0.12, 0.2, 0.28, 0.95);

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
    SubtabItems,
    SubtabProfiles,
    SubtabManage,
    QuantityUp,
    QuantityDown,
    QuantityMaxStack,
    CycleEndpoint,
    CycleEntry,
    AddItem,
    RemoveEntry,
    SetQuantity,
    ClearInventory,
    FillInventory,
    SetTransferSource,
    SetTransferDest,
    ExecuteTransfer,
    ArmPilePlacement,
    ValidateWorld,
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
                                    BackgroundColor(QTY_FIELD_BG_IDLE),
                                    BorderColor::all(QTY_FIELD_BORDER_IDLE),
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
                ("Catalog (I)", DevItemsAction::SubtabItems),
                ("Profiles (P)", DevItemsAction::SubtabProfiles),
                ("Manage (H)", DevItemsAction::SubtabManage),
                ("Target +", DevItemsAction::CycleEndpoint),
                ("Entry +", DevItemsAction::CycleEntry),
                ("Add (A)", DevItemsAction::AddItem),
                ("Remove (R)", DevItemsAction::RemoveEntry),
                ("Set qty (S)", DevItemsAction::SetQuantity),
                ("Clear (C)", DevItemsAction::ClearInventory),
                ("Fill (F)", DevItemsAction::FillInventory),
                ("Xfer src", DevItemsAction::SetTransferSource),
                ("Xfer dst", DevItemsAction::SetTransferDest),
                ("Xfer run", DevItemsAction::ExecuteTransfer),
                ("Spawn pile (G)", DevItemsAction::ArmPilePlacement),
                ("Validate (V)", DevItemsAction::ValidateWorld),
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
        BackgroundColor(QTY_BTN_BG),
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
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(QTY_BTN_BG),
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
    mut section: Query<&mut Visibility, With<DevItemsSection>>,
) {
    if !dev_state.is_changed() {
        return;
    }
    let show = dev_state.enabled && dev_state.active_tab == DevTab::Items;
    for mut visibility in &mut section {
        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn sync_item_quantity_controls(
    dev_state: Res<DevModeState>,
    items: Res<ItemCatalog>,
    mut qty_text: Query<&mut Text, With<DevItemQuantityText>>,
    mut max_stack_text: Query<&mut Text, (With<DevItemMaxStackText>, Without<DevItemQuantityText>)>,
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

    if let Ok(mut text) = qty_text.single_mut() {
        **text = display_qty;
    }

    if let Ok(mut text) = max_stack_text.single_mut() {
        **text = format_max_stack_label(dev_state.selected_definition.as_ref(), &items);
    }

    for (mut bg, mut border) in &mut qty_boxes {
        *bg = BackgroundColor(if focused {
            QTY_FIELD_BG_FOCUSED
        } else {
            QTY_FIELD_BG_IDLE
        });
        border.set_all(if focused {
            QTY_FIELD_BORDER_FOCUSED
        } else {
            QTY_FIELD_BORDER_IDLE
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
    inspector: Res<WorldInspectorState>,
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
        &inspector,
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
    inspector: Res<WorldInspectorState>,
    selection: Res<SelectedUnits>,
    unit_catalog: Res<UnitCatalog>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    pile_settings: Res<ItemPileSettings>,
    simulation: Res<SimulationControlState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    buttons: Query<(&Interaction, &DevItemsButton), Changed<Interaction>>,
    qty_boxes: Query<&Interaction, (With<DevItemQuantityBox>, Changed<Interaction>)>,
) {
    if !dev_state.enabled {
        return;
    }

    for interaction in &qty_boxes {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.focus_item_quantity();
        }
    }

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        match button.action {
            DevItemsAction::QuantityUp => dev_state.bump_item_quantity(1),
            DevItemsAction::QuantityDown => dev_state.bump_item_quantity(-1),
            DevItemsAction::QuantityMaxStack => dev_state.set_item_quantity_to_max_stack(&items),
            other => handle_dev_items_panel_action(
                &mut dev_state,
                &mut world,
                &inspector,
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
