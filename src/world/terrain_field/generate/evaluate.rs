//! Offline field value evaluation at global coordinates (ADR-102).

use super::super::id::{TerrainFieldId, TerrainFieldSourceProfileId};
use super::super::source::generator_config::{
    GeneratedTerrainFieldSource, TerrainFieldGeneratorDependency, TerrainFieldGeneratorKind,
};
use super::super::source_error::TerrainFieldSourceError;
use super::dependencies::{BiomeDependency, HeightfieldDependency};
use super::noise::{fbm_01, remap_to_u16, ridged_01, value_noise_01};
use super::seed::compose_field_seed;
use crate::world::BiomeId;

pub struct GenerationContext<'a> {
    pub field_id: &'a TerrainFieldId,
    pub profile_id: &'a TerrainFieldSourceProfileId,
    pub generated: &'a GeneratedTerrainFieldSource,
    pub heightfield: Option<&'a HeightfieldDependency>,
    pub biome: Option<&'a BiomeDependency>,
}

pub fn validate_generation_dependencies(
    generated: &GeneratedTerrainFieldSource,
    heightfield: Option<&HeightfieldDependency>,
    biome: Option<&BiomeDependency>,
) -> Result<(), TerrainFieldSourceError> {
    for dep in &generated.dependencies {
        match dep {
            TerrainFieldGeneratorDependency::Heightfield if heightfield.is_none() => {
                return Err(TerrainFieldSourceError::GeneratorDependencyMissing(
                    "Heightfield".to_string(),
                ));
            }
            TerrainFieldGeneratorDependency::BiomeMask if biome.is_none() => {
                return Err(TerrainFieldSourceError::GeneratorDependencyMissing(
                    "BiomeMask".to_string(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn generate_field_value(
    ctx: &GenerationContext<'_>,
    global_x_meters: f32,
    global_z_meters: f32,
) -> Result<u16, TerrainFieldSourceError> {
    validate_generation_dependencies(ctx.generated, ctx.heightfield, ctx.biome)?;
    let seed = compose_field_seed(
        ctx.generated.world_seed,
        ctx.field_id,
        ctx.profile_id,
        ctx.generated.generator_version,
    );
    let value_01 = match &ctx.generated.generator {
        TerrainFieldGeneratorKind::Constant { value } => {
            return Ok(*value);
        }
        TerrainFieldGeneratorKind::Gradient => {
            let t = (global_x_meters.fract().abs() + global_z_meters.fract().abs()) * 0.5;
            t % 1.0
        }
        TerrainFieldGeneratorKind::FractalNoise {
            scale_meters,
            octaves,
            persistence,
            lacunarity,
        } => fbm_01(
            seed,
            global_x_meters,
            global_z_meters,
            *scale_meters,
            *octaves,
            *persistence,
            *lacunarity,
        ),
        TerrainFieldGeneratorKind::GeologicalVeins {
            domain_scale_meters,
            vein_scale_meters,
            warp_strength,
            concentration_threshold,
            background,
            rich_value,
        } => geological_veins(
            seed,
            global_x_meters,
            global_z_meters,
            *domain_scale_meters,
            *vein_scale_meters,
            *warp_strength,
            *concentration_threshold,
            *background,
            *rich_value,
        ),
        TerrainFieldGeneratorKind::LowlandWaterPotential {
            aquifer_scale_meters,
            lowland_bias,
            mountain_suppression,
        } => lowland_water(
            seed,
            global_x_meters,
            global_z_meters,
            ctx.heightfield,
            *aquifer_scale_meters,
            *lowland_bias,
            *mountain_suppression,
        )?,
        TerrainFieldGeneratorKind::CopperPockets {
            pocket_scale_meters,
            pocket_density,
            background,
            rich_value,
        } => copper_pockets(
            seed,
            global_x_meters,
            global_z_meters,
            *pocket_scale_meters,
            *pocket_density,
            *background,
            *rich_value,
        ),
        TerrainFieldGeneratorKind::StoneExposure {
            elevation_weight,
            slope_weight,
            noise_scale_meters,
        } => stone_exposure(
            seed,
            global_x_meters,
            global_z_meters,
            ctx.heightfield,
            ctx.biome,
            *elevation_weight,
            *slope_weight,
            *noise_scale_meters,
        )?,
    };
    Ok(remap_to_u16(value_01))
}

fn geological_veins(
    seed: u64,
    x: f32,
    z: f32,
    domain_scale: f32,
    vein_scale: f32,
    warp_strength: f32,
    threshold: f32,
    background: u16,
    rich_value: u16,
) -> f32 {
    let wx = x
        + (value_noise_01(seed.wrapping_add(10), x, z, domain_scale) - 0.5)
            * domain_scale
            * warp_strength;
    let wz = z
        + (value_noise_01(seed.wrapping_add(20), x, z, domain_scale) - 0.5)
            * domain_scale
            * warp_strength;
    let domain = value_noise_01(seed.wrapping_add(30), wx, wz, domain_scale);
    let vein = ridged_01(seed.wrapping_add(40), wx, wz, vein_scale);
    let combined = (domain * 0.35 + vein * 0.65).clamp(0.0, 1.0);
    if combined < threshold {
        background as f32 / 65_535.0
    } else {
        let t = ((combined - threshold) / (1.0 - threshold)).clamp(0.0, 1.0);
        let bg = background as f32 / 65_535.0;
        let rich = rich_value as f32 / 65_535.0;
        bg + (rich - bg) * t
    }
}

fn lowland_water(
    seed: u64,
    x: f32,
    z: f32,
    heightfield: Option<&HeightfieldDependency>,
    aquifer_scale: f32,
    lowland_bias: f32,
    mountain_suppression: f32,
) -> Result<f32, TerrainFieldSourceError> {
    let Some(hf) = heightfield else {
        return Ok(0.0);
    };

    let Some(elev) = hf.normalized_elevation(x, z) else {
        return Ok(0.0);
    };
    let slope = hf.sample_slope_degrees(x, z).unwrap_or(0.0);
    let slope_t = (slope / 28.0).clamp(0.0, 1.0);

    // Peaks and steep faces drain — always dry regardless of noise.
    let peak_cutoff = (0.58 - mountain_suppression * 0.26).clamp(0.34, 0.58);
    if elev > peak_cutoff || slope_t > 0.30 {
        return Ok(0.0);
    }

    let lowland = (1.0 - elev / peak_cutoff).powf(2.4);
    let flat = (1.0 - slope_t).powf(2.0);
    let depression = hf.local_depression(x, z).unwrap_or(0.0);
    let terrain_capacity = lowland * flat * (0.50 + depression * 0.50);
    if terrain_capacity < 0.06 {
        return Ok(0.0);
    }

    // Noise only modulates how wet collectable low terrain is — not where mountains sit.
    let aquifer = fbm_01(seed, x, z, aquifer_scale, 4, 0.5, 2.0);
    let channels = ridged_01(seed.wrapping_add(40), x, z, aquifer_scale * 0.55);
    let wet_signal = (aquifer * 0.52 + channels * 0.48) * terrain_capacity;

    Ok(remap_water_response(wet_signal, lowland_bias))
}

/// Spread wet_signal across the full moisture range while preserving a hard dry floor.
fn remap_water_response(signal: f32, lowland_bias: f32) -> f32 {
    let dry_floor = (0.12 - lowland_bias * 0.20).clamp(0.05, 0.11);
    if signal <= dry_floor {
        return 0.0;
    }

    // Calibrated to observed main-world wet_signal (~0.10–0.38 on collectable lowland).
    let wet_ceiling = (0.34 - lowland_bias * 0.04).clamp(0.28, 0.34);
    let span = (wet_ceiling - dry_floor).max(1e-4);
    let t = ((signal - dry_floor) / span).clamp(0.0, 1.0);
    let body = remap_water_tier(t);

    if signal <= wet_ceiling {
        return body;
    }

    let tail_span = (0.50 - wet_ceiling).max(1e-4);
    let tail = ((signal - wet_ceiling) / tail_span).clamp(0.0, 1.0);
    let saturated = 0.80 + tail.powf(0.60) * 0.20;
    let blend = (tail * 2.0).clamp(0.0, 1.0);
    lerp_water(body, saturated, blend)
}

fn remap_water_tier(t: f32) -> f32 {
    const KNOTS: &[(f32, f32)] = &[
        (0.00, 0.01),
        (0.18, 0.04),
        (0.32, 0.08),
        (0.42, 0.10),
        (0.55, 0.16),
        (0.68, 0.24),
        (0.78, 0.34),
        (0.86, 0.46),
        (0.92, 0.58),
        (0.97, 0.70),
        (1.00, 0.80),
    ];
    piecewise_linear(t, KNOTS)
}

fn piecewise_linear(x: f32, knots: &[(f32, f32)]) -> f32 {
    if x <= knots[0].0 {
        return knots[0].1;
    }
    for window in knots.windows(2) {
        let (x0, y0) = window[0];
        let (x1, y1) = window[1];
        if x <= x1 {
            let u = (x - x0) / (x1 - x0);
            return y0 + u * (y1 - y0);
        }
    }
    knots.last().map(|(_, y)| *y).unwrap_or(0.0)
}

fn lerp_water(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn copper_pockets(
    seed: u64,
    x: f32,
    z: f32,
    pocket_scale: f32,
    pocket_density: f32,
    background: u16,
    rich_value: u16,
) -> f32 {
    let n1 = value_noise_01(seed.wrapping_add(100), x, z, pocket_scale);
    let n2 = value_noise_01(seed.wrapping_add(200), x, z, pocket_scale * 0.5);
    let pocket = (n1 * n2).powf(1.5);
    if pocket < pocket_density {
        background as f32 / 65_535.0
    } else {
        let t = ((pocket - pocket_density) / (1.0 - pocket_density)).clamp(0.0, 1.0);
        let bg = background as f32 / 65_535.0;
        let rich = rich_value as f32 / 65_535.0;
        bg + (rich - bg) * t
    }
}

fn stone_exposure(
    seed: u64,
    x: f32,
    z: f32,
    heightfield: Option<&HeightfieldDependency>,
    biome: Option<&BiomeDependency>,
    elevation_weight: f32,
    slope_weight: f32,
    noise_scale: f32,
) -> Result<f32, TerrainFieldSourceError> {
    let mut value = fbm_01(seed, x, z, noise_scale, 3, 0.5, 2.0) * 0.25;
    if let Some(hf) = heightfield {
        let height = hf.sample_height(x, z).unwrap_or(0.0);
        let slope = hf.sample_slope_degrees(x, z).unwrap_or(0.0);
        let elev_t = (height / 500.0).clamp(0.0, 1.0);
        let slope_t = (slope / 45.0).clamp(0.0, 1.0);
        value += elev_t * elevation_weight + slope_t * slope_weight;
    }
    if let Some(biome_dep) = biome {
        let sample = biome_dep
            .mask
            .sample_at_global(bevy::prelude::Vec3::new(x, 0.0, z));
        if matches!(sample.biome, BiomeId::Desert) {
            value += 0.15;
        }
    }
    Ok(value.clamp(0.0, 1.0))
}

pub fn generate_chunk_tile(
    ctx: &GenerationContext<'_>,
    chunk: crate::world::ChunkCoord,
    extent_min: crate::world::ChunkCoord,
    spacing: f32,
    bounds_origin_x: f32,
    bounds_origin_z: f32,
) -> Result<Vec<u16>, TerrainFieldSourceError> {
    use super::super::contract::TERRAIN_FIELD_SAMPLES_PER_EDGE;
    let intervals = super::super::contract::TERRAIN_FIELD_INTERVALS_PER_CHUNK as u32;
    let offset_x = (chunk.x - extent_min.x) as u32 * intervals;
    let offset_z = (chunk.z - extent_min.z) as u32 * intervals;
    let mut samples = Vec::with_capacity(
        (TERRAIN_FIELD_SAMPLES_PER_EDGE as usize) * (TERRAIN_FIELD_SAMPLES_PER_EDGE as usize),
    );
    for local_row in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
        for local_col in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
            let col = offset_x + local_col;
            let row = offset_z + local_row;
            let gx = bounds_origin_x + col as f32 * spacing;
            let gz = bounds_origin_z + row as f32 * spacing;
            samples.push(generate_field_value(ctx, gx, gz)?);
        }
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain_field::source::generator_config::TERRAIN_FIELD_GENERATOR_VERSION;

    fn ctx_for(
        kind: TerrainFieldGeneratorKind,
    ) -> (
        TerrainFieldId,
        TerrainFieldSourceProfileId,
        GeneratedTerrainFieldSource,
    ) {
        (
            TerrainFieldId::new("iron"),
            TerrainFieldSourceProfileId::new("test"),
            GeneratedTerrainFieldSource {
                generator: kind,
                generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
                world_seed: 99,
                dependencies: vec![],
            },
        )
    }

    #[test]
    fn same_seed_same_value() {
        let (field_id, profile_id, generated) =
            ctx_for(TerrainFieldGeneratorKind::GeologicalVeins {
                domain_scale_meters: 256.0,
                vein_scale_meters: 64.0,
                warp_strength: 0.3,
                concentration_threshold: 0.5,
                background: 1000,
                rich_value: 50000,
            });
        let ctx = GenerationContext {
            field_id: &field_id,
            profile_id: &profile_id,
            generated: &generated,
            heightfield: None,
            biome: None,
        };
        let a = generate_field_value(&ctx, 128.0, 256.0).unwrap();
        let b = generate_field_value(&ctx, 128.0, 256.0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn copper_differs_from_iron() {
        let (field_id, profile_id, iron) = ctx_for(TerrainFieldGeneratorKind::GeologicalVeins {
            domain_scale_meters: 256.0,
            vein_scale_meters: 64.0,
            warp_strength: 0.3,
            concentration_threshold: 0.5,
            background: 1000,
            rich_value: 50000,
        });
        let copper = GeneratedTerrainFieldSource {
            generator: TerrainFieldGeneratorKind::CopperPockets {
                pocket_scale_meters: 48.0,
                pocket_density: 0.2,
                background: 1000,
                rich_value: 50000,
            },
            generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
            world_seed: 99,
            dependencies: vec![],
        };
        let iron_ctx = GenerationContext {
            field_id: &field_id,
            profile_id: &profile_id,
            generated: &iron,
            heightfield: None,
            biome: None,
        };
        let copper_ctx = GenerationContext {
            field_id: &field_id,
            profile_id: &profile_id,
            generated: &copper,
            heightfield: None,
            biome: None,
        };
        let mut different = false;
        for i in 0..20 {
            let x = i as f32 * 37.0;
            let z = i as f32 * 53.0;
            if generate_field_value(&iron_ctx, x, z).unwrap()
                != generate_field_value(&copper_ctx, x, z).unwrap()
            {
                different = true;
                break;
            }
        }
        assert!(different);
    }

    #[test]
    fn water_response_preserves_dry_floor_and_reaches_rich_tail() {
        assert_eq!(remap_water_response(0.05, 0.02), 0.0);
        assert!(remap_water_response(0.40, 0.02) >= 0.80);
        let mid = remap_water_response(0.22, 0.02);
        assert!(mid > 0.05 && mid < 0.20, "mid was {mid}");
        let good = remap_water_response(0.30, 0.02);
        assert!(good >= 0.20 && good <= 0.55, "good was {good}");
    }

    #[test]
    fn peaks_are_dry_lowlands_can_be_wet() {
        use super::super::dependencies::HeightfieldDependency;
        use crate::world::{ChunkCoord, ChunkExtent, ChunkLayout};
        use std::collections::HashMap;

        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(0, 0),
        };
        let ramp_samples = (0..9)
            .map(|index| (index / 3) as f32 / 2.0 * 0.02)
            .collect();
        let ramp = crate::world::Heightfield::from_samples(3, 128.0, ramp_samples).unwrap();
        let plain = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        let mut tiles = HashMap::new();
        tiles.insert(ChunkCoord::new(0, 0), ramp);
        let hf = HeightfieldDependency::from_heightfields(layout, extent, tiles);

        let generated = GeneratedTerrainFieldSource {
            generator: TerrainFieldGeneratorKind::LowlandWaterPotential {
                aquifer_scale_meters: 384.0,
                lowland_bias: 0.02,
                mountain_suppression: 0.92,
            },
            generator_version: TERRAIN_FIELD_GENERATOR_VERSION,
            world_seed: 42_001,
            dependencies: vec![TerrainFieldGeneratorDependency::Heightfield],
        };
        let field_id = TerrainFieldId::new("water");
        let profile_id = TerrainFieldSourceProfileId::new("water_generated_v1");
        let ctx = GenerationContext {
            field_id: &field_id,
            profile_id: &profile_id,
            generated: &generated,
            heightfield: Some(&hf),
            biome: None,
        };

        let peak = generate_field_value(&ctx, 128.0, 240.0).unwrap();
        assert_eq!(peak, 0);

        let mut plain_tiles = HashMap::new();
        plain_tiles.insert(ChunkCoord::new(0, 0), plain);
        let flat_hf = HeightfieldDependency::from_heightfields(layout, extent, plain_tiles);
        let flat_ctx = GenerationContext {
            field_id: &field_id,
            profile_id: &profile_id,
            generated: &generated,
            heightfield: Some(&flat_hf),
            biome: None,
        };
        let mut any_wet = false;
        for offset in 0..12 {
            let value =
                generate_field_value(&flat_ctx, 32.0 + offset as f32 * 16.0, 128.0).unwrap();
            if value > 0 {
                any_wet = true;
                break;
            }
        }
        assert!(any_wet);
    }
}
