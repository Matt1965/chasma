use bevy::prelude::*;

use crate::world::WorldPosition;
use crate::world::doodad::catalog::DoodadDefinitionId;
use crate::world::doodad::generation::DoodadSpawnCandidate;
use crate::world::doodad::source::DoodadSource;

/// Exact transform to materialize into a [`crate::world::doodad::DoodadRecord`] (ADR-022).
///
/// Produced by placement finalization from a validated
/// [`DoodadSpawnCandidate`]. Generation candidates remain immutable; this type
/// carries the resolved world pose after terrain snapping.
///
/// Future extensions (normal alignment, random yaw/scale, ground offset) apply
/// here without mutating generation output.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct FinalizedDoodadPlacement {
    pub definition_id: DoodadDefinitionId,
    pub source: DoodadSource,
    pub position: WorldPosition,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl FinalizedDoodadPlacement {
    pub fn from_candidate(candidate: &DoodadSpawnCandidate) -> Self {
        Self {
            definition_id: candidate.definition_id.clone(),
            source: candidate.source,
            position: candidate.position,
            rotation: candidate.rotation,
            scale: candidate.scale,
        }
    }
}
