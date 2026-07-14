use bevy::prelude::*;

use super::id::ItemInstanceId;
use crate::world::ItemDefinitionId;

/// Index into [`super::record::InventoryRecord::placed_entries`].
pub type EntryIndex = usize;

/// One placed footprint in an inventory grid (ADR-088 I2).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct PlacedInventoryEntry {
    pub anchor_x: u8,
    pub anchor_y: u8,
    pub contents: InventoryEntryContents,
}

/// Stackable commodity vs unique instance contents (ADR-088 I2).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum InventoryEntryContents {
    Stack {
        item_definition_id: ItemDefinitionId,
        quantity: u32,
    },
    Unique {
        item_instance_id: ItemInstanceId,
    },
}

impl PlacedInventoryEntry {
    pub fn stack(
        anchor_x: u8,
        anchor_y: u8,
        item_definition_id: ItemDefinitionId,
        quantity: u32,
    ) -> Self {
        Self {
            anchor_x,
            anchor_y,
            contents: InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            },
        }
    }

    pub fn unique(anchor_x: u8, anchor_y: u8, item_instance_id: ItemInstanceId) -> Self {
        Self {
            anchor_x,
            anchor_y,
            contents: InventoryEntryContents::Unique { item_instance_id },
        }
    }
}
