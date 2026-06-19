use bevy::prelude::*;

use super::catalog::DoodadDefinitionId;
use super::generation::DoodadSpawnCandidate;
use super::placement::FinalizedDoodadPlacement;
use super::record::DoodadRecord;
use super::source::DoodadSource;
use crate::world::coordinates::ChunkCoord;

/// Stable identity for a procedurally generated doodad before [`super::id::DoodadId`] allocation (ADR-019).
///
/// Same chunk + definition + procedural seed always maps to the same key, enabling
/// idempotent materialization without scanning world records.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct ProceduralDoodadKey {
    pub chunk: ChunkCoord,
    pub definition_id: DoodadDefinitionId,
    pub procedural_seed: u64,
}

impl ProceduralDoodadKey {
    pub fn new(
        chunk: ChunkCoord,
        definition_id: DoodadDefinitionId,
        procedural_seed: u64,
    ) -> Self {
        Self {
            chunk,
            definition_id,
            procedural_seed,
        }
    }

    pub fn from_candidate(candidate: &DoodadSpawnCandidate) -> Option<Self> {
        match candidate.source {
            DoodadSource::Procedural { seed } => Some(Self {
                chunk: candidate.position.chunk,
                definition_id: candidate.definition_id.clone(),
                procedural_seed: seed,
            }),
            DoodadSource::Authored => None,
        }
    }

    pub fn from_finalized(placement: &FinalizedDoodadPlacement) -> Option<Self> {
        match placement.source {
            DoodadSource::Procedural { seed } => Some(Self {
                chunk: placement.position.chunk,
                definition_id: placement.definition_id.clone(),
                procedural_seed: seed,
            }),
            DoodadSource::Authored => None,
        }
    }

    pub fn from_record(record: &DoodadRecord) -> Option<Self> {
        match record.source {
            DoodadSource::Procedural { seed } => Some(Self {
                chunk: record.placement.position.chunk,
                definition_id: record.definition_id.clone(),
                procedural_seed: seed,
            }),
            DoodadSource::Authored => None,
        }
    }
}
