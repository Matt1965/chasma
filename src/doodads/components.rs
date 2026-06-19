use bevy::prelude::*;

use crate::world::{ChunkId, DoodadId};

/// Links a derived render entity to authoritative doodad data (ADR-023).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct DoodadRenderEntity {
    pub doodad_id: DoodadId,
    pub chunk_id: ChunkId,
}
