use bevy::prelude::*;

use crate::world::ItemPileId;
use crate::world::inventory::{EntryIndex, InventoryId, ItemInstanceId};

/// Authoritative location of a unique item instance (ADR-090 I4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum ItemInstanceLocation {
    #[default]
    Detached,
    Inventory {
        inventory_id: InventoryId,
        entry_index: EntryIndex,
    },
    WorldPile(ItemPileId),
}

impl ItemInstanceLocation {
    pub fn is_detached(self) -> bool {
        matches!(self, Self::Detached)
    }

    pub fn inventory(self) -> Option<(InventoryId, EntryIndex)> {
        match self {
            Self::Inventory {
                inventory_id,
                entry_index,
            } => Some((inventory_id, entry_index)),
            _ => None,
        }
    }

    pub fn world_pile(self) -> Option<ItemPileId> {
        match self {
            Self::WorldPile(id) => Some(id),
            _ => None,
        }
    }
}
