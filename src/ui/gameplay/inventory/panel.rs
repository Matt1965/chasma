//! Inventory panel presentation (ADR-092 I6).

use bevy::prelude::*;

use crate::client::inventory_intent::{
    InventoryIntent, InventoryIntentQueue, entry_revision_for_inventory,
};
use crate::ui::gameplay::inventory::state::{
    InventoryDragState, InventorySelection, InventoryUiState,
};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{
    BAR_BG, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY, hud_body_font, hud_title_font,
};
use crate::world::{
    InventoryCatalogCtx, InventoryEntryContents, InventoryId, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, ItemDefinitionId, ItemInstanceStore, WorldData, query_inventory_weight,
};

const CELL_PX: f32 = 28.0;

#[derive(Component, Debug)]
pub struct InventoryPanelRoot;

#[derive(Component, Debug)]
pub struct InventoryPanelCloseButton;

#[derive(Component, Debug)]
pub struct InventoryAutoSortButton {
    pub inventory_id: InventoryId,
}

#[derive(Component, Debug)]
pub struct InventoryLootAllButton;

#[derive(Component, Debug)]
pub struct InventoryPickupFullButton;

#[derive(Component, Debug, Clone, Copy)]
pub struct InventoryDepositGoldButton {
    pub amount: crate::client::inventory_intent::DepositGoldAmount,
}

#[derive(Component, Debug, Clone)]
pub struct InventoryGridPane {
    pub inventory_id: InventoryId,
    pub side: InventoryPaneSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryPaneSide {
    Left,
    Right,
}

#[derive(Component, Debug, Clone)]
pub struct InventoryGridCell {
    pub inventory_id: InventoryId,
    pub x: u8,
    pub y: u8,
    pub side: InventoryPaneSide,
}

#[derive(Component, Debug, Clone)]
pub struct InventoryEntryWidget {
    pub inventory_id: InventoryId,
    pub entry_index: usize,
    pub side: InventoryPaneSide,
}

#[derive(Component, Debug)]
pub struct InventoryHeaderText {
    pub side: InventoryPaneSide,
}

#[derive(Component, Debug)]
pub struct InventoryFeedbackText;

#[derive(Component, Debug)]
pub struct InventoryDetailsText;

#[derive(Component, Debug)]
pub struct InventoryEquipmentPlaceholder;

pub fn spawn_inventory_panel(mut commands: Commands) {
    commands
        .spawn((
            InventoryPanelRoot,
            PlayerHudUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(10.0),
                top: Val::Percent(8.0),
                width: Val::Percent(80.0),
                max_height: Val::Percent(84.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(8.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BG),
            ZIndex(400),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (
                        Text::new("Inventory"),
                        hud_title_font(),
                        TextColor(TEXT_PRIMARY),
                    ),
                    (
                        InventoryPanelCloseButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(PANEL_BG),
                        children![(
                            Text::new("Close"),
                            hud_body_font(),
                            TextColor(TEXT_PRIMARY),
                        )],
                    ),
                ],
            ));
            root.spawn((
                InventoryFeedbackText,
                Text::new(""),
                hud_body_font(),
                TextColor(TEXT_MUTED),
            ));
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    column_gap: Val::Px(12.0),
                    ..default()
                },
                InventoryDualPaneRow,
            ));
            root.spawn((
                InventoryDetailsText,
                Text::new("Select an item for details."),
                hud_body_font(),
                TextColor(TEXT_MUTED),
            ));
            root.spawn((
                InventoryEquipmentPlaceholder,
                Text::new("Equipment slots (Head, Body, Weapon, Offhand, Backpack) — not implemented in I6."),
                hud_body_font(),
                TextColor(TEXT_MUTED),
            ));
        });
}

#[derive(Component, Debug)]
pub(crate) struct InventoryDualPaneRow;

#[derive(Component, Debug)]
pub(crate) struct InventoryPaneContainer {
    side: InventoryPaneSide,
}

pub fn sync_inventory_panel_visibility(
    ui: Res<InventoryUiState>,
    mut query: Query<&mut Node, With<InventoryPanelRoot>>,
) {
    for mut node in &mut query {
        node.display = if ui.open {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub fn sync_inventory_panel_contents(
    ui: Res<InventoryUiState>,
    world: Res<WorldData>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    mut commands: Commands,
    row_query: Query<Entity, With<InventoryDualPaneRow>>,
    pane_query: Query<(Entity, &InventoryPaneContainer)>,
    mut feedback: Query<&mut Text, (With<InventoryFeedbackText>, Without<InventoryDetailsText>)>,
    mut details: Query<&mut Text, (With<InventoryDetailsText>, Without<InventoryFeedbackText>)>,
) {
    if !ui.is_changed() && !world.is_changed() {
        return;
    }
    if !ui.open {
        return;
    }
    let instance_store = world.item_instance_store();
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

    if let Ok(mut text) = feedback.single_mut() {
        **text = ui.feedback_message.clone();
    }

    if let Some(selection) = ui.selected {
        if let Ok(mut text) = details.single_mut() {
            **text = format_item_details(
                &world,
                &ctx,
                &items,
                &instance_store,
                selection.inventory_id,
                selection.entry_index,
            );
        }
    }

    let left_rev = ui
        .left_inventory_id
        .map(|id| inventory_revision(&world, id))
        .unwrap_or(0);
    let right_rev = ui
        .right_inventory_id
        .map(|id| inventory_revision(&world, id))
        .unwrap_or(0);
    if left_rev == ui.last_revision_left && right_rev == ui.last_revision_right && ui.is_changed() {
        // still refresh on pure ui selection changes below via is_changed only when needed
    }

    let Ok(row) = row_query.single() else {
        return;
    };
    for (entity, pane) in &pane_query {
        commands.entity(entity).despawn();
        let _ = pane;
    }
    commands.entity(row).with_children(|parent| {
        if let Some(left_id) = ui.left_inventory_id {
            spawn_pane(
                parent,
                &world,
                &ctx,
                &items,
                &instance_store,
                left_id,
                "Unit Inventory",
                InventoryPaneSide::Left,
                ui.treasury_deposit_open(),
            );
        }
        if let Some(treasury_id) = ui.treasury_id {
            spawn_treasury_pane(
                parent,
                &world,
                treasury_id,
                ui.secondary_label.as_deref().unwrap_or("Treasury"),
            );
        } else if let Some(right_id) = ui.right_inventory_id {
            let label = ui.secondary_label.as_deref().unwrap_or("Container");
            spawn_pane(
                parent,
                &world,
                &ctx,
                &items,
                &instance_store,
                right_id,
                label,
                InventoryPaneSide::Right,
                false,
            );
        } else if ui.pile_id.is_some() {
            parent.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                InventoryPaneContainer {
                    side: InventoryPaneSide::Right,
                },
                children![
                    (
                        Text::new("World Pile"),
                        hud_body_font(),
                        TextColor(TEXT_PRIMARY),
                    ),
                    (
                        InventoryPickupFullButton,
                        Button,
                        Node {
                            padding: UiRect::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(PANEL_BG),
                        children![(
                            Text::new("Pick Up Full"),
                            hud_body_font(),
                            TextColor(TEXT_PRIMARY),
                        )],
                    ),
                ],
            ));
        }
    });
}

fn spawn_pane(
    parent: &mut ChildSpawnerCommands<'_>,
    world: &WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    items: &ItemCatalog,
    instance_store: &ItemInstanceStore,
    inventory_id: InventoryId,
    title: &str,
    side: InventoryPaneSide,
    treasury_mode: bool,
) {
    let Some(record) = world.inventory_store().get(inventory_id) else {
        return;
    };
    let weight = query_inventory_weight(record, ctx)
        .map(|w| format_weight_line(&w))
        .unwrap_or_else(|_| "Weight unavailable".into());
    let gold = count_gold(record, items);
    let gold_line = if treasury_mode && side == InventoryPaneSide::Left {
        format!("Physical Gold: {gold}")
    } else {
        format!("Carried Gold: {gold}")
    };

    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                min_width: Val::Px(180.0),
                ..default()
            },
            InventoryPaneContainer { side },
        ))
        .with_children(|pane| {
            pane.spawn((
                InventoryHeaderText { side },
                Text::new(format!("{title}\n{weight}\n{gold_line}")),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
            pane.spawn((
                InventoryAutoSortButton { inventory_id },
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    align_self: AlignSelf::FlexStart,
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Auto-Sort"),
                    hud_body_font(),
                    TextColor(TEXT_PRIMARY),
                ));
            });
            if side == InventoryPaneSide::Right {
                pane.spawn((
                    InventoryLootAllButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        align_self: AlignSelf::FlexStart,
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Loot All"),
                        hud_body_font(),
                        TextColor(TEXT_PRIMARY),
                    ));
                });
            }
            pane.spawn((
                InventoryGridPane { inventory_id, side },
                Node {
                    width: Val::Px(CELL_PX * f32::from(record.grid_width())),
                    height: Val::Px(CELL_PX * f32::from(record.grid_height())),
                    position_type: PositionType::Relative,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.9)),
            ))
            .with_children(|grid| {
                for y in 0..record.grid_height() {
                    for x in 0..record.grid_width() {
                        grid.spawn((
                            InventoryGridCell {
                                inventory_id,
                                x,
                                y,
                                side,
                            },
                            Button,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(f32::from(x) * CELL_PX),
                                top: Val::Px(f32::from(y) * CELL_PX),
                                width: Val::Px(CELL_PX - 1.0),
                                height: Val::Px(CELL_PX - 1.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.15, 0.15, 0.18, 0.6)),
                        ));
                    }
                }
                for (entry_index, entry) in record.placed_entries().iter().enumerate() {
                    let (label, qty) = entry_label(entry, items, instance_store);
                    let (w, h) = entry_footprint(entry, items, instance_store);
                    grid.spawn((
                        InventoryEntryWidget {
                            inventory_id,
                            entry_index,
                            side,
                        },
                        Button,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(f32::from(entry.anchor_x) * CELL_PX),
                            top: Val::Px(f32::from(entry.anchor_y) * CELL_PX),
                            width: Val::Px(f32::from(w) * CELL_PX - 1.0),
                            height: Val::Px(f32::from(h) * CELL_PX - 1.0),
                            padding: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.25, 0.35, 0.55, 0.95)),
                    ))
                    .with_children(|item| {
                        item.spawn((
                            Text::new(if qty > 1 {
                                format!("{label}\n×{qty}")
                            } else {
                                label
                            }),
                            TextFont {
                                font_size: 10.0,
                                ..default()
                            },
                            TextColor(TEXT_PRIMARY),
                        ));
                    });
                }
            });
        });
}

fn spawn_treasury_pane(
    parent: &mut ChildSpawnerCommands<'_>,
    world: &WorldData,
    treasury_id: crate::world::TreasuryId,
    title: &str,
) {
    let balance = world
        .settlement_store()
        .get_treasury(treasury_id)
        .map(|t| t.balance_gold)
        .unwrap_or(0);
    use crate::client::inventory_intent::DepositGoldAmount;
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                min_width: Val::Px(180.0),
                ..default()
            },
            InventoryPaneContainer {
                side: InventoryPaneSide::Right,
            },
        ))
        .with_children(|pane| {
            pane.spawn((
                InventoryHeaderText {
                    side: InventoryPaneSide::Right,
                },
                Text::new(format!("{title}\nTreasury Gold: {balance}")),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
            for (label, amount) in [
                ("Deposit 1", DepositGoldAmount::One),
                ("Deposit Half", DepositGoldAmount::Half),
                ("Deposit All", DepositGoldAmount::All),
            ] {
                pane.spawn((
                    InventoryDepositGoldButton { amount },
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        align_self: AlignSelf::FlexStart,
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|btn| {
                    btn.spawn((Text::new(label), hud_body_font(), TextColor(TEXT_PRIMARY)));
                });
            }
        });
}

fn inventory_revision(world: &WorldData, inventory_id: InventoryId) -> u64 {
    world
        .inventory_store()
        .get(inventory_id)
        .map(|r| r.placed_entries().len() as u64 * 10_000 + r.total_mass_grams())
        .unwrap_or(0)
}

fn count_gold(record: &crate::world::InventoryRecord, items: &ItemCatalog) -> u32 {
    let gold_id = ItemDefinitionId::new("gold");
    record
        .placed_entries()
        .iter()
        .filter_map(|entry| match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } if item_definition_id == &gold_id => Some(*quantity),
            _ => None,
        })
        .sum()
}

fn format_weight_line(query: &crate::world::InventoryWeightQuery) -> String {
    let total_kg = query.total_mass_grams as f64 / 1000.0;
    let reference = query
        .reference_weight_grams
        .map(|g| format!("{:.1} kg ref", g as f64 / 1000.0))
        .unwrap_or_else(|| "no ref".into());
    let burden = if query.over_reference_grams > 0 {
        " · heavy"
    } else {
        ""
    };
    format!("{total_kg:.1} kg ({reference}){burden}")
}

fn entry_label(
    entry: &crate::world::PlacedInventoryEntry,
    items: &ItemCatalog,
    instance_store: &ItemInstanceStore,
) -> (String, u32) {
    match &entry.contents {
        InventoryEntryContents::Stack {
            item_definition_id,
            quantity,
        } => {
            let name = items
                .get(item_definition_id)
                .map(|d| d.display_name.clone())
                .unwrap_or_else(|| item_definition_id.as_str().to_string());
            (name, *quantity)
        }
        InventoryEntryContents::Unique { item_instance_id } => {
            let name = instance_store
                .get(*item_instance_id)
                .map(|i| {
                    items
                        .get(&i.definition_id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or_else(|| i.definition_id.as_str().to_string())
                })
                .unwrap_or_else(|| "Unique".into());
            (name, 1)
        }
    }
}

fn entry_footprint(
    entry: &crate::world::PlacedInventoryEntry,
    items: &ItemCatalog,
    instance_store: &ItemInstanceStore,
) -> (u8, u8) {
    let def_id = match &entry.contents {
        InventoryEntryContents::Stack {
            item_definition_id, ..
        } => item_definition_id.clone(),
        InventoryEntryContents::Unique { item_instance_id } => instance_store
            .get(*item_instance_id)
            .map(|i| i.definition_id.clone())
            .unwrap_or_else(|| ItemDefinitionId::new("unknown")),
    };
    items
        .get(&def_id)
        .map(|d| (d.grid_width, d.grid_height))
        .unwrap_or((1, 1))
}

fn format_item_details(
    world: &WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    items: &ItemCatalog,
    instance_store: &ItemInstanceStore,
    inventory_id: InventoryId,
    entry_index: usize,
) -> String {
    let Some(record) = world.inventory_store().get(inventory_id) else {
        return "Inventory missing.".into();
    };
    let Some(entry) = record.placed_entries().get(entry_index) else {
        return "Entry missing.".into();
    };
    let (name, qty) = entry_label(entry, items, instance_store);
    let def_id = match &entry.contents {
        InventoryEntryContents::Stack {
            item_definition_id, ..
        } => item_definition_id.clone(),
        InventoryEntryContents::Unique { item_instance_id } => instance_store
            .get(*item_instance_id)
            .map(|i| i.definition_id.clone())
            .unwrap_or_else(|| ItemDefinitionId::new("unknown")),
    };
    let Some(def) = items.get(&def_id) else {
        return format!("{name} — missing definition");
    };
    let mass = def.mass_grams_per_unit.saturating_mul(qty);
    let weight = query_inventory_weight(record, ctx)
        .map(|q| format!("{:.1} kg total inv", q.total_mass_grams as f64 / 1000.0))
        .unwrap_or_default();
    format!(
        "{name}\n{}\nCategory: {}\nSize: {}×{}\nQty: {qty}\nMass: {:.2} kg\nValue: {} gold\nTags: {}\n{weight}",
        def.description,
        def.category_id.as_str(),
        def.grid_width,
        def.grid_height,
        mass as f64 / 1000.0,
        def.base_value_gold,
        def.tags.join(", "),
    )
}

pub fn handle_inventory_panel_buttons(
    ui: Res<InventoryUiState>,
    mut queue: ResMut<InventoryIntentQueue>,
    close: Query<&Interaction, (Changed<Interaction>, With<InventoryPanelCloseButton>)>,
    sort: Query<(&Interaction, &InventoryAutoSortButton), Changed<Interaction>>,
    loot_all: Query<&Interaction, (Changed<Interaction>, With<InventoryLootAllButton>)>,
    pickup: Query<&Interaction, (Changed<Interaction>, With<InventoryPickupFullButton>)>,
    deposit: Query<(&Interaction, &InventoryDepositGoldButton), Changed<Interaction>>,
) {
    if close.iter().any(|i| *i == Interaction::Pressed) {
        queue.push(InventoryIntent::Close);
        return;
    }
    for (interaction, button) in &sort {
        if *interaction == Interaction::Pressed {
            queue.push(InventoryIntent::AutoSort {
                inventory_id: button.inventory_id,
            });
        }
    }
    if loot_all.iter().any(|i| *i == Interaction::Pressed) {
        if let (Some(actor), Some(corpse_inv), Some(dest)) = (
            ui.actor_unit_id,
            ui.right_inventory_id,
            ui.left_inventory_id,
        ) {
            queue.push(InventoryIntent::LootAll {
                corpse_inventory_id: corpse_inv,
                actor_unit_id: actor,
                destination_inventory_id: dest,
            });
        }
    }
    if pickup.iter().any(|i| *i == Interaction::Pressed) {
        if let (Some(actor), Some(pile_id)) = (ui.actor_unit_id, ui.pile_id) {
            queue.push(InventoryIntent::PickupPile {
                pile_id,
                actor_unit_id: actor,
                quantity: None,
            });
        }
    }
    for (interaction, button) in &deposit {
        if *interaction == Interaction::Pressed {
            if let (Some(actor), Some(treasury_id)) = (ui.actor_unit_id, ui.treasury_id) {
                queue.push(InventoryIntent::DepositGold {
                    treasury_id,
                    actor_unit_id: actor,
                    amount: button.amount,
                });
            }
        }
    }
}

pub fn handle_inventory_entry_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<InventoryUiState>,
    world: Res<WorldData>,
    mut queue: ResMut<InventoryIntentQueue>,
    entries: Query<(&Interaction, &InventoryEntryWidget), Changed<Interaction>>,
    cells: Query<(&Interaction, &InventoryGridCell), Changed<Interaction>>,
) {
    if !ui.open {
        return;
    }
    let ctrl = keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    for (interaction, widget) in &entries {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let revision =
            entry_revision_for_inventory(world.as_ref(), widget.inventory_id, widget.entry_index);
        ui.selected = Some(InventorySelection {
            inventory_id: widget.inventory_id,
            entry_index: widget.entry_index,
        });

        let other_inventory = if widget.side == InventoryPaneSide::Left {
            ui.right_inventory_id
        } else {
            ui.left_inventory_id
        };

        if mouse.just_pressed(MouseButton::Right) {
            let _ = interaction;
            continue;
        }

        if ctrl {
            if let Some(dest) = other_inventory {
                queue.push(InventoryIntent::TransferOne {
                    source_inventory_id: widget.inventory_id,
                    source_entry_index: widget.entry_index,
                    destination_inventory_id: dest,
                    entry_revision: revision,
                });
            }
            continue;
        }
        if shift {
            if let Some(dest) = other_inventory {
                queue.push(InventoryIntent::TransferHalf {
                    source_inventory_id: widget.inventory_id,
                    source_entry_index: widget.entry_index,
                    destination_inventory_id: dest,
                    entry_revision: revision,
                });
            }
            continue;
        }
        if ui.dragging.is_none() {
            ui.dragging = Some(InventoryDragState {
                source_inventory_id: widget.inventory_id,
                entry_index: widget.entry_index,
                entry_revision: revision,
            });
        }
    }

    for (interaction, cell) in &cells {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(drag) = ui.dragging.clone() else {
            continue;
        };
        if drag.source_inventory_id == cell.inventory_id {
            queue.push(InventoryIntent::MoveEntry {
                inventory_id: cell.inventory_id,
                entry_index: drag.entry_index,
                anchor_x: cell.x,
                anchor_y: cell.y,
                entry_revision: drag.entry_revision,
            });
        } else if let Some(dest) = Some(cell.inventory_id) {
            queue.push(InventoryIntent::TransferToCell {
                source_inventory_id: drag.source_inventory_id,
                source_entry_index: drag.entry_index,
                destination_inventory_id: dest,
                anchor_x: cell.x,
                anchor_y: cell.y,
                entry_revision: drag.entry_revision,
            });
        }
        ui.dragging = None;
    }
}

pub fn collect_inventory_mouse_transfers(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    ui: Res<InventoryUiState>,
    world: Res<WorldData>,
    mut queue: ResMut<InventoryIntentQueue>,
    entries: Query<&InventoryEntryWidget>,
    interactions: Query<(&Interaction, &InventoryEntryWidget)>,
) {
    if !ui.open || !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let ctrl = keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if ctrl || shift {
        return;
    }
    let Some((_, widget)) = interactions
        .iter()
        .find(|(state, _)| **state == Interaction::Hovered || **state == Interaction::Pressed)
    else {
        return;
    };
    let revision =
        entry_revision_for_inventory(world.as_ref(), widget.inventory_id, widget.entry_index);
    let other = if widget.side == InventoryPaneSide::Left {
        ui.right_inventory_id
    } else {
        ui.left_inventory_id
    };
    let Some(dest) = other else {
        return;
    };
    queue.push(InventoryIntent::TransferFull {
        source_inventory_id: widget.inventory_id,
        source_entry_index: widget.entry_index,
        destination_inventory_id: dest,
        entry_revision: revision,
    });
    let _ = entries;
}
