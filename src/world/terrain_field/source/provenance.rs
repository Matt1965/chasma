//! Source provenance recorded on packaged field layers (ADR-102).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::id::{TerrainFieldId, TerrainFieldSourceProfileId};
use super::bounds::TerrainFieldWorldBounds;
use super::generator_config::TerrainFieldGeneratorKind;
use crate::world::{ChunkCoord, ChunkExtent};

/// Serializable chunk extent for provenance manifests (ADR-102).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceChunkExtent {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

impl From<ChunkExtent> for ProvenanceChunkExtent {
    fn from(extent: ChunkExtent) -> Self {
        Self {
            min_x: extent.min.x,
            min_z: extent.min.z,
            max_x: extent.max.x,
            max_z: extent.max.z,
        }
    }
}

impl From<ProvenanceChunkExtent> for ChunkExtent {
    fn from(extent: ProvenanceChunkExtent) -> Self {
        Self {
            min: ChunkCoord::new(extent.min_x, extent.min_z),
            max: ChunkCoord::new(extent.max_x, extent.max_z),
        }
    }
}

/// Deterministic provenance for a built field package (ADR-102).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldSourceProvenance {
    pub profile_id: TerrainFieldSourceProfileId,
    pub field_id: TerrainFieldId,
    pub profile_revision: String,
    pub generator_kind: Option<String>,
    pub generator_version: u32,
    pub world_seed: u64,
    pub input_asset_hashes: Vec<String>,
    pub target_resolution: (u32, u32),
    pub world_extent: ProvenanceChunkExtent,
    pub world_bounds: TerrainFieldWorldBounds,
}

impl TerrainFieldSourceProvenance {
    pub fn source_version_hash(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.profile_id.as_str().hash(&mut hasher);
        self.profile_revision.hash(&mut hasher);
        self.generator_kind.hash(&mut hasher);
        self.generator_version.hash(&mut hasher);
        self.world_seed.hash(&mut hasher);
        for h in &self.input_asset_hashes {
            h.hash(&mut hasher);
        }
        self.target_resolution.hash(&mut hasher);
        self.world_extent.min_x.hash(&mut hasher);
        self.world_extent.min_z.hash(&mut hasher);
        self.world_extent.max_x.hash(&mut hasher);
        self.world_extent.max_z.hash(&mut hasher);
        format!("tf2_{:016x}", hasher.finish())
    }
}

pub fn generator_kind_label(kind: &TerrainFieldGeneratorKind) -> &'static str {
    match kind {
        TerrainFieldGeneratorKind::Constant { .. } => "Constant",
        TerrainFieldGeneratorKind::Gradient => "Gradient",
        TerrainFieldGeneratorKind::FractalNoise { .. } => "FractalNoise",
        TerrainFieldGeneratorKind::GeologicalVeins { .. } => "GeologicalVeins",
        TerrainFieldGeneratorKind::LowlandWaterPotential { .. } => "LowlandWaterPotential",
        TerrainFieldGeneratorKind::CopperPockets { .. } => "CopperPockets",
        TerrainFieldGeneratorKind::StoneExposure { .. } => "StoneExposure",
    }
}
