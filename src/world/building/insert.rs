use bevy::prelude::*;

use super::id::BuildingId;

/// Why a building insert, move, or remove failed (ADR-079 B2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingInsertError {
    ChunkPlacementMismatch,
    BuildingNotFound,
}
