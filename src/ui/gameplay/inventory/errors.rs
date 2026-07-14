//! User-facing inventory UI error messages (ADR-092 I6).

use crate::world::BuildingInventoryError;
use crate::world::{InventoryError, ItemPileError, TransferError};
use crate::world::{TreasuryAccessResult, TreasuryError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryUiError {
    NoRoom,
    StackFull,
    ItemChanged,
    InventoryClosed,
    AccessDenied,
    OutOfRange,
    WrongSpace,
    ContainerLocked,
    InvalidTargetCell,
    CannotMerge,
    CannotSwap,
    QuantityUnavailable,
    ItemAlreadyMoved,
    AutoSortFailed,
    MissingItemData,
    PileGone,
    CorpseGone,
    TreasuryUnavailable,
    UnitHasNoInventory,
    Other(String),
}

impl InventoryUiError {
    pub fn message(self) -> String {
        match self {
            Self::NoRoom => "No room in destination inventory.".into(),
            Self::StackFull => "Destination stack is full.".into(),
            Self::ItemChanged => "Item changed — try again.".into(),
            Self::InventoryClosed => "Inventory is no longer available.".into(),
            Self::AccessDenied => "Access denied.".into(),
            Self::OutOfRange => "Out of interaction range.".into(),
            Self::WrongSpace => "Wrong floor or space.".into(),
            Self::ContainerLocked => "Container is locked.".into(),
            Self::InvalidTargetCell => "Cannot place item there.".into(),
            Self::CannotMerge => "Stacks cannot merge.".into(),
            Self::CannotSwap => "Cannot swap items — not enough space.".into(),
            Self::QuantityUnavailable => "Requested quantity unavailable.".into(),
            Self::ItemAlreadyMoved => "Item was already moved.".into(),
            Self::AutoSortFailed => "Auto-sort failed.".into(),
            Self::MissingItemData => "Missing item definition.".into(),
            Self::PileGone => "World pile is gone.".into(),
            Self::CorpseGone => "Corpse is gone.".into(),
            Self::TreasuryUnavailable => "Treasury is no longer available.".into(),
            Self::UnitHasNoInventory => "This unit has no inventory.".into(),
            Self::Other(msg) => msg,
        }
    }

    pub fn from_transfer(error: TransferError) -> Self {
        match error {
            TransferError::DestinationNoFit => Self::NoRoom,
            TransferError::SourceEntryMissing { .. } => Self::ItemChanged,
            TransferError::SourceInventoryNotFound(_)
            | TransferError::DestinationInventoryNotFound(_) => Self::InventoryClosed,
            TransferError::InvalidTransferQuantity { .. } => Self::QuantityUnavailable,
            TransferError::TransferPartialNotAllowed { .. } => Self::StackFull,
            TransferError::Inventory(inv) => Self::from_inventory(inv),
            other => Self::Other(other.to_string()),
        }
    }

    pub fn from_inventory(error: InventoryError) -> Self {
        match error {
            InventoryError::InventoryNotFound(_) => Self::InventoryClosed,
            InventoryError::EntryNotFound { .. } => Self::ItemChanged,
            InventoryError::GridOutOfBounds { .. } | InventoryError::CellsOccupied { .. } => {
                Self::InvalidTargetCell
            }
            InventoryError::InvalidSwap { .. } => Self::CannotSwap,
            InventoryError::StackLimitExceeded { .. } => Self::StackFull,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn from_pile(error: ItemPileError) -> Self {
        Self::Other(error.to_string())
    }

    pub fn from_building(error: BuildingInventoryError) -> Self {
        match error {
            BuildingInventoryError::InventoryAccessDenied
            | BuildingInventoryError::ContainerLocked(_) => Self::ContainerLocked,
            BuildingInventoryError::BuildingNotOperational(_) => Self::AccessDenied,
            BuildingInventoryError::OutOfRange => Self::OutOfRange,
            BuildingInventoryError::WrongSpace => Self::WrongSpace,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn from_treasury(error: TreasuryError) -> Self {
        match error {
            TreasuryError::AccessDenied => Self::AccessDenied,
            TreasuryError::OutOfRange => Self::OutOfRange,
            TreasuryError::WrongSpace => Self::WrongSpace,
            TreasuryError::InsufficientPhysicalGold { .. } => Self::QuantityUnavailable,
            TreasuryError::TreasuryNotFound(_)
            | TreasuryError::SettlementNotFound(_)
            | TreasuryError::BuildingNotFound(_)
            | TreasuryError::BuildingNotSettlementCapable(_) => Self::TreasuryUnavailable,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn from_treasury_access(result: TreasuryAccessResult) -> Self {
        match result {
            TreasuryAccessResult::Allowed => Self::AccessDenied,
            TreasuryAccessResult::Denied(error) => Self::from_treasury(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_no_fit_maps_to_no_room() {
        let err = InventoryUiError::from_transfer(TransferError::DestinationNoFit);
        assert_eq!(err, InventoryUiError::NoRoom);
    }

    #[test]
    fn treasury_insufficient_gold_maps_to_quantity_unavailable() {
        use crate::world::TreasuryError;
        let err = InventoryUiError::from_treasury(TreasuryError::InsufficientPhysicalGold {
            available: 1,
            requested: 5,
        });
        assert_eq!(err, InventoryUiError::QuantityUnavailable);
    }
}
