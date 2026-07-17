//! Terrain field spatial contract (ADR-101 TF1).

use crate::world::WorldConfig;

/// Meters between stored field samples along each chunk axis.
pub const TERRAIN_FIELD_SAMPLE_SPACING_METERS: f32 = 8.0;

/// Shared-edge samples per chunk edge at the TF1 resolution.
pub const TERRAIN_FIELD_SAMPLES_PER_EDGE: u16 = 33;

/// Sample intervals per chunk edge (256 m / 8 m).
pub const TERRAIN_FIELD_INTERVALS_PER_CHUNK: u16 = 32;

/// Total samples per tile.
pub const TERRAIN_FIELD_SAMPLES_PER_TILE: usize =
    (TERRAIN_FIELD_SAMPLES_PER_EDGE as usize) * (TERRAIN_FIELD_SAMPLES_PER_EDGE as usize);

/// Expected tile byte size (`u16` samples).
pub const TERRAIN_FIELD_BYTES_PER_TILE: usize = TERRAIN_FIELD_SAMPLES_PER_TILE * 2;

/// Validate that world config matches the TF1 field resolution contract.
pub fn validate_world_config_for_fields(
    config: &WorldConfig,
) -> Result<(), TerrainFieldContractError> {
    if (config.chunk_size_meters - 256.0).abs() > 1e-3 {
        return Err(TerrainFieldContractError::UnsupportedChunkSize {
            found: config.chunk_size_meters,
            expected: 256.0,
        });
    }
    let intervals = config.chunk_size_meters / TERRAIN_FIELD_SAMPLE_SPACING_METERS;
    let rounded = intervals.round();
    if (intervals - rounded).abs() > 1e-4 || rounded as u16 != TERRAIN_FIELD_INTERVALS_PER_CHUNK {
        return Err(TerrainFieldContractError::SampleSpacingMismatch {
            chunk_size_meters: config.chunk_size_meters,
            sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
        });
    }
    Ok(())
}

/// Expected samples per edge for the active contract.
pub fn expected_samples_per_edge(_config: &WorldConfig) -> u16 {
    TERRAIN_FIELD_SAMPLES_PER_EDGE
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerrainFieldContractError {
    UnsupportedChunkSize {
        found: f32,
        expected: f32,
    },
    SampleSpacingMismatch {
        chunk_size_meters: f32,
        sample_spacing_meters: f32,
    },
}

impl std::fmt::Display for TerrainFieldContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedChunkSize { found, expected } => {
                write!(
                    f,
                    "terrain fields require {expected} m chunks, found {found} m"
                )
            }
            Self::SampleSpacingMismatch {
                chunk_size_meters,
                sample_spacing_meters,
            } => write!(
                f,
                "chunk size {chunk_size_meters} m is not divisible by field sample spacing {sample_spacing_meters} m"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldConfig;

    #[test]
    fn default_world_config_matches_contract() {
        validate_world_config_for_fields(&WorldConfig::default()).unwrap();
    }
}
