//! Treasury and deposit errors (ADR-093 I7).

use std::fmt;

use super::id::{SettlementId, TreasuryId};
use crate::world::{BuildingId, InventoryError, InventoryId, UnitId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreasuryError {
    TreasuryNotFound(TreasuryId),
    SettlementNotFound(SettlementId),
    BuildingNotFound(BuildingId),
    BuildingNotSettlementCapable(BuildingId),
    SettlementAlreadyExists(BuildingId),
    DuplicateTreasuryId(TreasuryId),
    DuplicateSettlementId(SettlementId),
    BuildingAlreadyLinked(BuildingId),
    RequesterMissing(UnitId),
    SourceInventoryNotFound(InventoryId),
    SourceInventoryNotOwnedByUnit {
        inventory_id: InventoryId,
        unit_id: UnitId,
    },
    AccessDenied,
    OutOfRange,
    WrongSpace,
    InvalidQuantity {
        requested: u32,
    },
    InsufficientPhysicalGold {
        available: u32,
        requested: u32,
    },
    QuantityOverflow,
    Inventory(InventoryError),
}

impl fmt::Display for TreasuryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TreasuryNotFound(id) => write!(f, "treasury {id:?} not found"),
            Self::SettlementNotFound(id) => write!(f, "settlement {id:?} not found"),
            Self::BuildingNotFound(id) => write!(f, "building {id:?} not found"),
            Self::BuildingNotSettlementCapable(id) => {
                write!(f, "building {id:?} cannot host a settlement treasury")
            }
            Self::SettlementAlreadyExists(id) => {
                write!(f, "building {id:?} already has a settlement")
            }
            Self::DuplicateTreasuryId(id) => write!(f, "duplicate treasury id {id:?}"),
            Self::DuplicateSettlementId(id) => write!(f, "duplicate settlement id {id:?}"),
            Self::BuildingAlreadyLinked(id) => {
                write!(f, "building {id:?} already linked to another settlement")
            }
            Self::RequesterMissing(id) => write!(f, "unit {id:?} not found"),
            Self::SourceInventoryNotFound(id) => write!(f, "inventory {id:?} not found"),
            Self::SourceInventoryNotOwnedByUnit {
                inventory_id,
                unit_id,
            } => write!(
                f,
                "inventory {inventory_id:?} is not owned by unit {unit_id:?}"
            ),
            Self::AccessDenied => write!(f, "treasury access denied"),
            Self::OutOfRange => write!(f, "out of treasury interaction range"),
            Self::WrongSpace => write!(f, "wrong space for treasury deposit"),
            Self::InvalidQuantity { requested } => {
                write!(f, "invalid deposit quantity {requested}")
            }
            Self::InsufficientPhysicalGold {
                available,
                requested,
            } => write!(
                f,
                "insufficient physical gold (have {available}, need {requested})"
            ),
            Self::QuantityOverflow => write!(f, "treasury balance overflow"),
            Self::Inventory(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TreasuryError {}

impl From<InventoryError> for TreasuryError {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}
