//! Client-local inventory panel state (ADR-092 I6).

use bevy::prelude::*;

use crate::client::inventory_intent::InventoryOpenMode;
use crate::world::{
    CorpseId, EntryIndex, InventoryId, ItemPileId, SettlementId, TreasuryId, UnitId,
};

/// Drag payload — not authoritative.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryDragState {
    pub source_inventory_id: InventoryId,
    pub entry_index: EntryIndex,
    pub entry_revision: u64,
}

/// Selected entry for details panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventorySelection {
    pub inventory_id: InventoryId,
    pub entry_index: EntryIndex,
}

/// Optional explicit split dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitDialogState {
    pub inventory_id: InventoryId,
    pub entry_index: EntryIndex,
    pub quantity: u32,
    pub max_quantity: u32,
}

/// Client-local inventory UI state — never WorldData truth.
#[derive(Resource, Debug, Clone, Default)]
pub struct InventoryUiState {
    pub open: bool,
    pub mode: Option<InventoryOpenMode>,
    pub actor_unit_id: Option<UnitId>,
    pub left_inventory_id: Option<InventoryId>,
    pub right_inventory_id: Option<InventoryId>,
    pub secondary_label: Option<String>,
    pub pile_id: Option<ItemPileId>,
    pub corpse_id: Option<CorpseId>,
    pub treasury_id: Option<TreasuryId>,
    pub settlement_id: Option<SettlementId>,
    pub treasury_building_id: Option<crate::world::BuildingId>,
    pub selected: Option<InventorySelection>,
    pub dragging: Option<InventoryDragState>,
    pub split_dialog: Option<SplitDialogState>,
    pub feedback_message: String,
    pub last_revision_left: u64,
    pub last_revision_right: u64,
}

impl InventoryUiState {
    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn open_mode(&mut self, mode: InventoryOpenMode) {
        self.open = true;
        self.mode = Some(mode.clone());
        self.selected = None;
        self.dragging = None;
        self.split_dialog = None;
        self.feedback_message.clear();
        match mode {
            InventoryOpenMode::UnitOnly { unit_id } => {
                self.actor_unit_id = Some(unit_id);
                self.left_inventory_id = None;
                self.right_inventory_id = None;
                self.secondary_label = None;
                self.pile_id = None;
                self.corpse_id = None;
                self.treasury_id = None;
                self.settlement_id = None;
                self.treasury_building_id = None;
            }
            InventoryOpenMode::DualTransfer {
                actor_unit_id,
                secondary_inventory_id,
                secondary_label,
            } => {
                self.actor_unit_id = Some(actor_unit_id);
                self.right_inventory_id = Some(secondary_inventory_id);
                self.secondary_label = Some(secondary_label);
                self.pile_id = None;
                self.corpse_id = None;
                self.treasury_id = None;
                self.settlement_id = None;
                self.treasury_building_id = None;
            }
            InventoryOpenMode::WorldPile {
                actor_unit_id,
                pile_id,
            } => {
                self.actor_unit_id = Some(actor_unit_id);
                self.pile_id = Some(pile_id);
                self.right_inventory_id = None;
                self.secondary_label = Some("World Pile".to_string());
                self.corpse_id = None;
                self.treasury_id = None;
                self.settlement_id = None;
                self.treasury_building_id = None;
            }
            InventoryOpenMode::TreasuryDeposit {
                actor_unit_id,
                treasury_id,
                settlement_id,
                building_id,
                label,
            } => {
                self.actor_unit_id = Some(actor_unit_id);
                self.treasury_id = Some(treasury_id);
                self.settlement_id = Some(settlement_id);
                self.treasury_building_id = Some(building_id);
                self.secondary_label = Some(label);
                self.right_inventory_id = None;
                self.pile_id = None;
                self.corpse_id = None;
            }
        }
    }

    pub fn treasury_deposit_open(&self) -> bool {
        self.treasury_id.is_some()
    }

    pub fn invalidate_drag(&mut self) {
        self.dragging = None;
    }

    pub fn dual_transfer_open(&self) -> bool {
        self.right_inventory_id.is_some() && self.pile_id.is_none()
    }
}
