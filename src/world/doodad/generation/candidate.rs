use bevy::prelude::*;

use super::super::catalog::DoodadDefinitionId;
use super::super::source::DoodadSource;
use crate::world::WorldPosition;

/// A procedural doodad that *would* exist in a chunk (ADR-018).
///
/// Not a world instance: no [`super::super::id::DoodadId`], no metadata, no
/// chunk-store ownership. A later phase materializes candidates into
/// [`crate::world::WorldData`] via the authoring API.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct DoodadSpawnCandidate {
    pub definition_id: DoodadDefinitionId,
    pub source: DoodadSource,
    pub position: WorldPosition,
    pub rotation: Quat,
    pub scale: Vec3,
}
