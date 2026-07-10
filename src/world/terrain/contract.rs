//! Heightfield contract validation against world config (REVIEW-B6, ADR-067).

use crate::world::WorldConfig;

use super::{Heightfield, TerrainDataError};

/// Absolute tolerance for comparing sample spacing to config (meters).
pub const SPACING_TOLERANCE_METERS: f32 = 1e-5;

/// Relative tolerance for comparing derived chunk span to config.
pub const CHUNK_SPAN_RELATIVE_TOLERANCE: f32 = 1e-5;

/// Expected samples per edge for a chunk under the world terrain contract.
pub fn expected_samples_per_edge(config: &WorldConfig) -> Result<u32, TerrainDataError> {
    let spacing = config.meters_per_sample;
    if !(spacing > 0.0) || !spacing.is_finite() {
        return Err(TerrainDataError::NonPositiveSpacing { spacing_meters: spacing });
    }
    let chunk_size = config.chunk_size_meters;
    if !(chunk_size > 0.0) || !chunk_size.is_finite() {
        return Err(TerrainDataError::NonPositiveSpacing {
            spacing_meters: chunk_size,
        });
    }
    let spans = chunk_size / spacing;
    let rounded = spans.round();
    if (spans - rounded).abs() > SPACING_TOLERANCE_METERS {
        return Err(TerrainDataError::InvalidDimensions {
            expected_samples_per_edge: 0,
            found_samples_per_edge: 0,
        });
    }
    Ok(rounded as u32 + 1)
}

/// Validate that a heightfield matches the authoritative world terrain contract.
pub fn validate_heightfield_against_config(
    heightfield: &Heightfield,
    config: &WorldConfig,
) -> Result<(), TerrainDataError> {
    let expected_spe = expected_samples_per_edge(config)?;
    let found_spe = heightfield.samples_per_edge();
    if found_spe != expected_spe {
        return Err(TerrainDataError::InvalidDimensions {
            expected_samples_per_edge: expected_spe,
            found_samples_per_edge: found_spe,
        });
    }

    let expected_spacing = config.meters_per_sample;
    let found_spacing = heightfield.spacing_meters();
    if (found_spacing - expected_spacing).abs() > SPACING_TOLERANCE_METERS {
        return Err(TerrainDataError::SpacingMismatch {
            expected_meters: expected_spacing,
            found_meters: found_spacing,
        });
    }

    let expected_span = config.chunk_size_meters;
    let found_span = heightfield.chunk_size_meters();
    if found_span < expected_span * (1.0 - CHUNK_SPAN_RELATIVE_TOLERANCE)
        || found_span > expected_span * (1.0 + CHUNK_SPAN_RELATIVE_TOLERANCE)
    {
        return Err(TerrainDataError::InvalidDimensions {
            expected_samples_per_edge: expected_spe,
            found_samples_per_edge: found_spe,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldConfig;

    #[test]
    fn rejects_spacing_mismatch() {
        let config = WorldConfig {
            meters_per_sample: 128.0,
            ..WorldConfig::default()
        };
        let hf = Heightfield::from_samples(3, 64.0, vec![0.0; 9]).unwrap();
        assert!(matches!(
            validate_heightfield_against_config(&hf, &config),
            Err(TerrainDataError::SpacingMismatch { .. })
        ));
    }

    #[test]
    fn accepts_matching_contract() {
        let config = WorldConfig {
            meters_per_sample: 128.0,
            ..WorldConfig::default()
        };
        let hf = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        assert!(validate_heightfield_against_config(&hf, &config).is_ok());
    }
}
