use bevy::prelude::*;

use crate::world::{BuildingId, BuildingLifecycleState, ChunkId};

/// Links a derived render entity to authoritative building data (ADR-079 B2).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct BuildingRenderEntity {
    pub building_id: BuildingId,
    pub chunk_id: ChunkId,
    pub lifecycle_state: BuildingLifecycleState,
}
