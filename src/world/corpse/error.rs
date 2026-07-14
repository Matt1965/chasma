use super::id::CorpseId;
use crate::world::inventory::InventoryId;
use crate::world::unit::UnitId;

/// Structured corpse errors (ADR-089 I3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpseError {
    CorpseNotFound(CorpseId),
    CorpseIdCollision(CorpseId),
    DuplicateOriginUnit(UnitId),
    ChunkPlacementMismatch {
        corpse_id: CorpseId,
    },
    CorpseInventoryTransferFailed {
        unit_id: UnitId,
        inventory_id: InventoryId,
        message: String,
    },
    CorpseLifetimeInvalid {
        corpse_id: CorpseId,
    },
    CorpseRemovalFailed {
        corpse_id: CorpseId,
    },
    ContainedItemCleanupFailed {
        inventory_id: InventoryId,
    },
    DeathInventoryInvariantViolation {
        unit_id: UnitId,
        message: String,
    },
}

impl std::fmt::Display for CorpseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorpseNotFound(id) => write!(f, "corpse not found `{id:?}`"),
            Self::CorpseIdCollision(id) => write!(f, "corpse id collision `{id:?}`"),
            Self::DuplicateOriginUnit(id) => write!(f, "duplicate corpse for unit `{id:?}`"),
            Self::ChunkPlacementMismatch { corpse_id } => {
                write!(f, "corpse `{corpse_id:?}` chunk mismatch")
            }
            Self::CorpseInventoryTransferFailed {
                unit_id,
                inventory_id,
                message,
            } => write!(
                f,
                "corpse inventory transfer failed unit `{unit_id:?}` inventory `{inventory_id:?}`: {message}"
            ),
            Self::CorpseLifetimeInvalid { corpse_id } => {
                write!(f, "invalid corpse lifetime for `{corpse_id:?}`")
            }
            Self::CorpseRemovalFailed { corpse_id } => {
                write!(f, "corpse removal failed `{corpse_id:?}`")
            }
            Self::ContainedItemCleanupFailed { inventory_id } => write!(
                f,
                "contained item cleanup failed for inventory `{inventory_id:?}`"
            ),
            Self::DeathInventoryInvariantViolation { unit_id, message } => write!(
                f,
                "death inventory invariant violation for unit `{unit_id:?}`: {message}"
            ),
        }
    }
}

impl std::error::Error for CorpseError {}
