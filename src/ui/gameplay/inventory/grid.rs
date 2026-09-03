//! Shared inventory grid cell/item presentation (BP2 extraction from unit inventory panel).

use bevy::prelude::*;

use crate::ui::gameplay::inventory::drag_preview::source_entry_drag_color;
use crate::ui::gameplay::inventory::preview::INVENTORY_CELL_PX;
use crate::ui::gameplay::inventory::state::InventoryUiState;
use crate::ui::gameplay::styles::{TEXT_PRIMARY, hud_body_font};
use crate::world::{
    InventoryEntryContents, InventoryId, InventoryRecord, ItemCatalog, ItemDefinitionId,
    ItemInstanceStore, PlacedInventoryEntry,
};

const CELL_PX: f32 = INVENTORY_CELL_PX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryPaneSide {
    Left,
    Right,
}

#[derive(Component, Debug, Clone)]
pub struct InventoryGridPane {
    pub inventory_id: InventoryId,
    pub side: InventoryPaneSide,
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

/// Whether grid cells and items accept player inventory interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryGridInteraction {
    /// BP2 building panel: same visuals, no drag/transfer components.
    ReadOnly,
    /// Unit/container inventory panel (ADR-092 I6).
    Interactive { side: InventoryPaneSide },
}

/// Marker on read-only building-panel grids (regression tests / diagnostics).
#[derive(Component, Debug, Clone, Copy)]
pub struct ReadOnlyInventoryGrid;

/// Spawn one authoritative inventory grid with cells and placed entries.
pub fn spawn_inventory_grid(
    parent: &mut ChildSpawnerCommands<'_>,
    record: &InventoryRecord,
    inventory_id: InventoryId,
    items: &ItemCatalog,
    instance_store: &ItemInstanceStore,
    interaction: InventoryGridInteraction,
    ui: Option<&InventoryUiState>,
) {
    let interactive = matches!(interaction, InventoryGridInteraction::Interactive { .. });
    let side = match interaction {
        InventoryGridInteraction::ReadOnly => InventoryPaneSide::Left,
        InventoryGridInteraction::Interactive { side } => side,
    };

    let mut grid_entity = parent.spawn((
        Node {
            width: Val::Px(CELL_PX * f32::from(record.grid_width())),
            height: Val::Px(CELL_PX * f32::from(record.grid_height())),
            position_type: PositionType::Relative,
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.9)),
    ));
    if !interactive {
        grid_entity.insert(ReadOnlyInventoryGrid);
    }

    grid_entity.with_children(|grid| {
        for y in 0..record.grid_height() {
            for x in 0..record.grid_width() {
                let cell_node = Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(f32::from(x) * CELL_PX),
                    top: Val::Px(f32::from(y) * CELL_PX),
                    width: Val::Px(CELL_PX - 1.0),
                    height: Val::Px(CELL_PX - 1.0),
                    ..default()
                };
                let cell_bg = BackgroundColor(Color::srgba(0.15, 0.15, 0.18, 0.6));
                if interactive {
                    grid.spawn((
                        InventoryGridCell {
                            inventory_id,
                            x,
                            y,
                            side,
                        },
                        Button,
                        cell_node,
                        cell_bg,
                    ));
                } else {
                    grid.spawn((cell_node, cell_bg));
                }
            }
        }

        for (entry_index, entry) in record.placed_entries().iter().enumerate() {
            let (label, qty) = entry_label(entry, items, instance_store);
            let (w, h) = entry_footprint(entry, items, instance_store);
            let item_node = Node {
                position_type: PositionType::Absolute,
                left: Val::Px(f32::from(entry.anchor_x) * CELL_PX),
                top: Val::Px(f32::from(entry.anchor_y) * CELL_PX),
                width: Val::Px(f32::from(w) * CELL_PX - 1.0),
                height: Val::Px(f32::from(h) * CELL_PX - 1.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            };
            let base_color = Color::srgba(0.25, 0.35, 0.55, 0.95);
            let item_bg = if interactive {
                BackgroundColor(source_entry_drag_color(
                    ui.expect("interactive grid requires InventoryUiState"),
                    inventory_id,
                    entry_index,
                    base_color,
                ))
            } else {
                BackgroundColor(base_color)
            };
            let text = if qty > 1 {
                format!("{label}\n×{qty}")
            } else {
                label
            };
            if interactive {
                grid.spawn((
                    InventoryEntryWidget {
                        inventory_id,
                        entry_index,
                        side,
                    },
                    Button,
                    item_node,
                    item_bg,
                ))
                .with_children(|item| {
                    item.spawn((
                        Text::new(text),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                    ));
                });
            } else {
                grid.spawn((item_node, item_bg)).with_children(|item| {
                    item.spawn((
                        Text::new(text),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                    ));
                });
            }
        }
    });
}

pub(crate) fn entry_label(
    entry: &PlacedInventoryEntry,
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

pub(crate) fn entry_footprint(
    entry: &PlacedInventoryEntry,
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

/// Read-only empty grid shell when a binding exists but inventory runtime is not yet loaded.
pub fn spawn_read_only_inventory_grid_shell(
    parent: &mut ChildSpawnerCommands<'_>,
    grid_width: u8,
    grid_height: u8,
) {
    parent
        .spawn((
            Node {
                width: Val::Px(CELL_PX * f32::from(grid_width)),
                height: Val::Px(CELL_PX * f32::from(grid_height)),
                position_type: PositionType::Relative,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.9)),
            ReadOnlyInventoryGrid,
        ))
        .with_children(|grid| {
            for y in 0..grid_height {
                for x in 0..grid_width {
                    grid.spawn((
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
        });
}
