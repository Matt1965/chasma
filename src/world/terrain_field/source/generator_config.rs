//! Generator kinds and dependency declarations (ADR-102).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Offline generator profile kinds (ADR-102).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TerrainFieldGeneratorKind {
    Constant {
        value: u16,
    },
    Gradient,
    FractalNoise {
        scale_meters: f32,
        octaves: u8,
        persistence: f32,
        lacunarity: f32,
    },
    GeologicalVeins {
        domain_scale_meters: f32,
        vein_scale_meters: f32,
        warp_strength: f32,
        concentration_threshold: f32,
        background: u16,
        rich_value: u16,
    },
    LowlandWaterPotential {
        aquifer_scale_meters: f32,
        lowland_bias: f32,
        mountain_suppression: f32,
    },
    CopperPockets {
        pocket_scale_meters: f32,
        pocket_density: f32,
        background: u16,
        rich_value: u16,
    },
    StoneExposure {
        elevation_weight: f32,
        slope_weight: f32,
        noise_scale_meters: f32,
    },
}

pub const TERRAIN_FIELD_GENERATOR_VERSION: u32 = 1;

/// Declared dependencies for offline field generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainFieldGeneratorDependency {
    Heightfield,
    BiomeMask,
}

/// Generated field source configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedTerrainFieldSource {
    pub generator: TerrainFieldGeneratorKind,
    pub generator_version: u32,
    pub world_seed: u64,
    pub dependencies: Vec<TerrainFieldGeneratorDependency>,
}

impl Default for GeneratedTerrainFieldSource {
    fn default() -> Self {
        Self {
            generator: TerrainFieldGeneratorKind::Constant { value: 0 },
            generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
            world_seed: 1,
            dependencies: Vec::new(),
        }
    }
}
