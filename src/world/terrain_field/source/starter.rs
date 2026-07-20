//! Initial generated source profiles for TF2 (ADR-102).

use super::generator_config::{
    GeneratedTerrainFieldSource, TERRAIN_FIELD_GENERATOR_VERSION, TerrainFieldGeneratorDependency,
    TerrainFieldGeneratorKind,
};
use super::profile::TerrainFieldSourceProfileDefinition;

pub fn starter_source_profiles() -> Vec<TerrainFieldSourceProfileDefinition> {
    vec![
        water_profile(),
        iron_profile(),
        copper_profile(),
        stone_profile(),
    ]
}

fn water_profile() -> TerrainFieldSourceProfileDefinition {
    TerrainFieldSourceProfileDefinition::generated(
        "water_generated_v1",
        "Generated Water Potential",
        "water",
        GeneratedTerrainFieldSource {
            generator: TerrainFieldGeneratorKind::LowlandWaterPotential {
                aquifer_scale_meters: 384.0,
                lowland_bias: 0.02,
                mountain_suppression: 0.92,
            },
            generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
            world_seed: 42_001,
            dependencies: vec![TerrainFieldGeneratorDependency::Heightfield],
        },
    )
}

fn iron_profile() -> TerrainFieldSourceProfileDefinition {
    TerrainFieldSourceProfileDefinition::generated(
        "iron_generated_v1",
        "Generated Iron Veins",
        "iron",
        GeneratedTerrainFieldSource {
            generator: TerrainFieldGeneratorKind::GeologicalVeins {
                domain_scale_meters: 768.0,
                vein_scale_meters: 96.0,
                warp_strength: 0.45,
                concentration_threshold: 0.62,
                background: 4_000,
                rich_value: 58_000,
            },
            generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
            world_seed: 42_002,
            dependencies: vec![],
        },
    )
}

fn copper_profile() -> TerrainFieldSourceProfileDefinition {
    TerrainFieldSourceProfileDefinition::generated(
        "copper_generated_v1",
        "Generated Copper Pockets",
        "copper",
        GeneratedTerrainFieldSource {
            generator: TerrainFieldGeneratorKind::CopperPockets {
                pocket_scale_meters: 64.0,
                pocket_density: 0.18,
                background: 2_500,
                rich_value: 52_000,
            },
            generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
            world_seed: 42_003,
            dependencies: vec![],
        },
    )
}

fn stone_profile() -> TerrainFieldSourceProfileDefinition {
    TerrainFieldSourceProfileDefinition::generated(
        "stone_generated_v1",
        "Generated Stone Exposure",
        "stone",
        GeneratedTerrainFieldSource {
            generator: TerrainFieldGeneratorKind::StoneExposure {
                elevation_weight: 0.55,
                slope_weight: 0.35,
                noise_scale_meters: 256.0,
            },
            generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
            world_seed: 42_004,
            dependencies: vec![
                TerrainFieldGeneratorDependency::Heightfield,
                TerrainFieldGeneratorDependency::BiomeMask,
            ],
        },
    )
}
