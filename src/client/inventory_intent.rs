//! Inventory UI intents — client requests before authoritative mutation (ADR-092 I6).

use bevy::prelude::*;

use crate::world::{
    CorpseId, EntryIndex, InventoryId, ItemPileId, SettlementId, TreasuryId, UnitId,
};

/// How the inventory panel was opened (client-local presentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryOpenMode {
    UnitOnly {
        unit_id: UnitId,
    },
    DualTransfer {
        actor_unit_id: UnitId,
        secondary_inventory_id: InventoryId,
        secondary_label: String,
    },
    WorldPile {
        actor_unit_id: UnitId,
        pile_id: ItemPileId,
    },
    TreasuryDeposit {
        actor_unit_id: UnitId,
        treasury_id: TreasuryId,
        settlement_id: SettlementId,
        building_id: crate::world::BuildingId,
        label: String,
    },
}

/// Client-side inventory action awaiting authoritative dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum InventoryIntent {
    Open(InventoryOpenMode),
    Close,
    MoveEntry {
        inventory_id: InventoryId,
        entry_index: EntryIndex,
        anchor_x: u8,
        anchor_y: u8,
        entry_revision: u64,
    },
    TransferFull {
        source_inventory_id: InventoryId,
        source_entry_index: EntryIndex,
        destination_inventory_id: InventoryId,
        entry_revision: u64,
    },
    TransferOne {
        source_inventory_id: InventoryId,
        source_entry_index: EntryIndex,
        destination_inventory_id: InventoryId,
        entry_revision: u64,
    },
    TransferHalf {
        source_inventory_id: InventoryId,
        source_entry_index: EntryIndex,
        destination_inventory_id: InventoryId,
        entry_revision: u64,
    },
    TransferToCell {
        source_inventory_id: InventoryId,
        source_entry_index: EntryIndex,
        destination_inventory_id: InventoryId,
        anchor_x: u8,
        anchor_y: u8,
        entry_revision: u64,
    },
    AutoSort {
        inventory_id: InventoryId,
    },
    DropEntry {
        inventory_id: InventoryId,
        entry_index: EntryIndex,
        actor_unit_id: UnitId,
        entry_revision: u64,
    },
    PickupPile {
        pile_id: ItemPileId,
        actor_unit_id: UnitId,
        quantity: Option<u32>,
    },
    LootAll {
        corpse_inventory_id: InventoryId,
        actor_unit_id: UnitId,
        destination_inventory_id: InventoryId,
    },
    DepositGold {
        treasury_id: TreasuryId,
        actor_unit_id: UnitId,
        amount: DepositGoldAmount,
    },
}

/// How much physical gold to deposit from the actor inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositGoldAmount {
    One,
    Half,
    All,
}

/// Outcome of dispatching one inventory intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryIntentStatus {
    Applied,
    Ignored,
    Rejected,
}

#[derive(Resource, Default, Debug)]
pub struct InventoryIntentQueue {
    pending: Vec<InventoryIntent>,
}

impl InventoryIntentQueue {
    pub fn push(&mut self, intent: InventoryIntent) {
        self.pending.push(intent);
    }

    pub fn drain(&mut self) -> Vec<InventoryIntent> {
        std::mem::take(&mut self.pending)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Stable revision for stale drag detection (entry count + mass).
pub fn entry_revision_for_inventory(
    world: &crate::world::WorldData,
    inventory_id: InventoryId,
    entry_index: EntryIndex,
) -> u64 {
    let Some(record) = world.inventory_store().get(inventory_id) else {
        return 0;
    };
    let entry_mass = record
        .placed_entries()
        .get(entry_index)
        .map(|entry| match &entry.contents {
            crate::world::InventoryEntryContents::Stack { quantity, .. } => u64::from(*quantity),
            crate::world::InventoryEntryContents::Unique { .. } => 1,
        })
        .unwrap_or(0);
    record.placed_entries().len() as u64 * 10_000 + record.total_mass_grams() + entry_mass
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_drains_in_order() {
        let mut queue = InventoryIntentQueue::default();
        queue.push(InventoryIntent::Close);
        queue.push(InventoryIntent::AutoSort {
            inventory_id: InventoryId::new(1),
        });
        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(matches!(drained[0], InventoryIntent::Close));
    }
}
