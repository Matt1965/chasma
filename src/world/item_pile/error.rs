use super::id::ItemPileId;
use crate::world::inventory::{InventoryId, ItemInstanceId};
use crate::world::{ItemDefinitionId, SpaceId};

/// Structured world pile errors (ADR-090 I4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemPileError {
    ItemPileNotFound(ItemPileId),
    ItemPileIdCollision(ItemPileId),
    ChunkPlacementMismatch {
        pile_id: ItemPileId,
    },
    IncompatiblePileMerge {
        source: ItemPileId,
        target: ItemPileId,
    },
    WorldPileStackLimitExceeded {
        item_definition_id: ItemDefinitionId,
        requested: u32,
        limit: u32,
    },
    PilePlacementUnavailable,
    WrongSpace {
        expected: SpaceId,
        actual: SpaceId,
    },
    Unauthorized,
    QuantityOverflow,
    DropRollbackFailed,
    PickupRollbackFailed,
    MergePlanInvalid(String),
    ItemInstanceLocationMismatch {
        item_instance_id: ItemInstanceId,
    },
    CorpseInventoryMissing {
        inventory_id: InventoryId,
    },
}

impl std::fmt::Display for ItemPileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ItemPileNotFound(id) => write!(f, "item pile not found `{id}`"),
            Self::ItemPileIdCollision(id) => write!(f, "item pile id collision `{id}`"),
            Self::ChunkPlacementMismatch { pile_id } => {
                write!(f, "chunk placement mismatch for pile `{pile_id}`")
            }
            Self::IncompatiblePileMerge { source, target } => {
                write!(f, "incompatible merge {source} -> {target}")
            }
            Self::WorldPileStackLimitExceeded {
                item_definition_id,
                requested,
                limit,
            } => write!(
                f,
                "world pile stack limit for `{}`: requested {requested}, limit {limit}",
                item_definition_id.as_str()
            ),
            Self::PilePlacementUnavailable => write!(f, "pile placement unavailable"),
            Self::WrongSpace { expected, actual } => {
                write!(f, "wrong space expected {expected:?}, got {actual:?}")
            }
            Self::Unauthorized => write!(f, "unauthorized pile access"),
            Self::QuantityOverflow => write!(f, "quantity overflow"),
            Self::DropRollbackFailed => write!(f, "drop rollback failed"),
            Self::PickupRollbackFailed => write!(f, "pickup rollback failed"),
            Self::MergePlanInvalid(msg) => write!(f, "merge plan invalid: {msg}"),
            Self::ItemInstanceLocationMismatch { item_instance_id } => {
                write!(f, "item instance location mismatch `{item_instance_id:?}`")
            }
            Self::CorpseInventoryMissing { inventory_id } => {
                write!(f, "corpse inventory missing `{inventory_id:?}`")
            }
        }
    }
}

impl std::error::Error for ItemPileError {}
