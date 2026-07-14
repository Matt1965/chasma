use super::id::BuildingId;
use crate::world::inventory::{InventoryError, InventoryId};
use crate::world::{InventoryProfileId, ItemPileError};

/// Structured building container inventory errors (ADR-091 I5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingInventoryError {
    BuildingNotFound(BuildingId),
    BuildingHasNoInventory(BuildingId),
    InventoryProfileMissing {
        building_id: BuildingId,
        profile_id: InventoryProfileId,
    },
    BuildingInventoryOwnerMismatch {
        building_id: BuildingId,
        inventory_id: InventoryId,
    },
    BuildingNotOperational(BuildingId),
    InventoryAccessDenied,
    InteractionPointMissing(BuildingId),
    WrongSpace,
    OutOfRange,
    ContainerLocked(BuildingId),
    SpillPlanningFailed(String),
    SpillPlacementUnavailable,
    InventoryCleanupFailed(InventoryId),
    RemovalPolicyMissing,
    ContentsStillPresent {
        building_id: BuildingId,
        inventory_id: InventoryId,
    },
    OrphanedBuildingInventory {
        building_id: BuildingId,
        inventory_id: InventoryId,
    },
    Inventory(InventoryError),
    ItemPile(ItemPileError),
}

impl From<InventoryError> for BuildingInventoryError {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl From<ItemPileError> for BuildingInventoryError {
    fn from(value: ItemPileError) -> Self {
        Self::ItemPile(value)
    }
}

impl std::fmt::Display for BuildingInventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildingNotFound(id) => write!(f, "building not found `{id:?}`"),
            Self::BuildingHasNoInventory(id) => write!(f, "building `{id:?}` has no inventory"),
            Self::InventoryProfileMissing {
                building_id,
                profile_id,
            } => write!(
                f,
                "building `{building_id:?}` missing profile `{profile_id:?}`"
            ),
            Self::BuildingInventoryOwnerMismatch {
                building_id,
                inventory_id,
            } => write!(
                f,
                "building `{building_id:?}` owner mismatch for inventory `{inventory_id:?}`"
            ),
            Self::BuildingNotOperational(id) => write!(f, "building `{id:?}` not operational"),
            Self::InventoryAccessDenied => write!(f, "inventory access denied"),
            Self::InteractionPointMissing(id) => {
                write!(f, "building `{id:?}` missing inventory interaction point")
            }
            Self::WrongSpace => write!(f, "wrong space for container access"),
            Self::OutOfRange => write!(f, "out of interaction range"),
            Self::ContainerLocked(id) => write!(f, "container locked on building `{id:?}`"),
            Self::SpillPlanningFailed(msg) => write!(f, "spill planning failed: {msg}"),
            Self::SpillPlacementUnavailable => write!(f, "spill placement unavailable"),
            Self::InventoryCleanupFailed(id) => {
                write!(f, "inventory cleanup failed `{id:?}`")
            }
            Self::RemovalPolicyMissing => write!(f, "removal policy missing"),
            Self::ContentsStillPresent {
                building_id,
                inventory_id,
            } => write!(
                f,
                "contents remain in `{inventory_id:?}` on building `{building_id:?}`"
            ),
            Self::OrphanedBuildingInventory {
                building_id,
                inventory_id,
            } => write!(
                f,
                "orphaned inventory `{inventory_id:?}` for building `{building_id:?}`"
            ),
            Self::Inventory(error) => write!(f, "{error}"),
            Self::ItemPile(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for BuildingInventoryError {}
