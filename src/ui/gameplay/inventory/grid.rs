//! Shared inventory grid cell/item presentation (BP2 extraction from unit inventory panel).

use bevy::prelude::*;

use crate::ui::gameplay::inventory::drag_preview::source_entry_drag_color;
use crate::ui::gameplay::inventory::preview::INVENTORY_CELL_PX;
use crate::ui::gameplay::inventory::state::InventoryUiState;
use crate::ui::gameplay::styles::{
    HUD_RECESSED_CORE, HUD_RECESSED_FACE, HUD_ROSTER_SLOT_BORDER, TEXT_PRIMARY, panel_body_font,
};
use crate::world::{
    InventoryEntryContents, InventoryId, InventoryRecord, ItemCatalog, ItemDefinitionId,
    ItemInstanceStore, PlacedInventoryEntry,
};

const CELL_PX: f32 = INVENTORY_CELL_PX;
const INVENTORY_CELL_BORDER_PX: f32 = 1.0;
const INVENTORY_GRID_OUTER_BORDER_PX: f32 = 1.0;

/// Recessed inventory well with a slightly stronger bronze outer rim.
fn inventory_grid_shell_style() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_RECESSED_FACE),
        BorderColor::all(Color::srgba(0.48, 0.39, 0.30, 0.58)),
    )
}

/// Per-slot outline — muted bronze on charcoal so every cell reads at gameplay distance.
fn inventory_grid_cell_style() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_RECESSED_CORE),
        BorderColor::all(HUD_ROSTER_SLOT_BORDER),
    )
}

fn inventory_grid_cell_node(x: u8, y: u8) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(f32::from(x) * CELL_PX),
        top: Val::Px(f32::from(y) * CELL_PX),
        width: Val::Px(CELL_PX),
        height: Val::Px(CELL_PX),
        border: UiRect::all(Val::Px(INVENTORY_CELL_BORDER_PX)),
        ..default()
    }
}

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

    let (grid_bg, grid_border) = inventory_grid_shell_style();
    let mut grid_entity = parent.spawn((
        Node {
            width: Val::Px(CELL_PX * f32::from(record.grid_width())),
            height: Val::Px(CELL_PX * f32::from(record.grid_height())),
            position_type: PositionType::Relative,
            flex_shrink: 0.0,
            border: UiRect::all(Val::Px(INVENTORY_GRID_OUTER_BORDER_PX)),
            ..default()
        },
        grid_bg,
        grid_border,
    ));
    if !interactive {
        grid_entity.insert(ReadOnlyInventoryGrid);
    }

    grid_entity.with_children(|grid| {
        for y in 0..record.grid_height() {
            for x in 0..record.grid_width() {
                let cell_node = inventory_grid_cell_node(x, y);
                let (cell_bg, cell_border) = inventory_grid_cell_style();
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
                        cell_border,
                    ));
                } else {
                    grid.spawn((cell_node, cell_bg, cell_border));
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
                format!("{label}\nx{qty}")
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
    let (grid_bg, grid_border) = inventory_grid_shell_style();
    parent
        .spawn((
            Node {
                width: Val::Px(CELL_PX * f32::from(grid_width)),
                height: Val::Px(CELL_PX * f32::from(grid_height)),
                position_type: PositionType::Relative,
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(INVENTORY_GRID_OUTER_BORDER_PX)),
                ..default()
            },
            grid_bg,
            grid_border,
            ReadOnlyInventoryGrid,
        ))
        .with_children(|grid| {
            for y in 0..grid_height {
                for x in 0..grid_width {
                    let (cell_bg, cell_border) = inventory_grid_cell_style();
                    grid.spawn((inventory_grid_cell_node(x, y), cell_bg, cell_border));
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_grid_cells_declare_visible_hairline_borders() {
        let node = inventory_grid_cell_node(2, 1);
        assert!(matches!(
            node.border.left,
            Val::Px(px) if px >= INVENTORY_CELL_BORDER_PX
        ));
        assert_eq!(node.width, Val::Px(CELL_PX));
        assert_eq!(node.height, Val::Px(CELL_PX));
    }

    #[test]
    fn inventory_grid_cell_borders_use_warm_charcoal_palette() {
        let (_, cell_border) = inventory_grid_cell_style();
        let edge = cell_border.left.to_srgba();
        assert!(
            edge.alpha > 0.35,
            "cell borders must be visible at gameplay distance"
        );
        assert!(
            edge.red > edge.blue,
            "inventory grid lines should read bronze/charcoal, not cyan"
        );
        let (_, shell_border) = inventory_grid_shell_style();
        let rim = shell_border.left.to_srgba();
        assert!(
            rim.alpha > 0.4,
            "grid outer rim should be slightly stronger than cells"
        );
    }
}
