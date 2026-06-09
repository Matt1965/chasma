use bevy::prelude::*;

use super::TerrainDataError;

/// One imported terrain mask layer for a chunk, as plain per-sample data
/// (ADR-003, ADR-008).
///
/// Masks are data only in Phase 1. Their consumers (terrain material blending in
/// Phase 2, doodad placement in Phase 3) do not exist yet, so this is a data
/// seam with no processing system (AGENTS.md Groundwork Rule). A mask may use a
/// different resolution than the heightfield.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TerrainMask {
    /// Identifier of the layer this mask represents (e.g. "grass", "rock").
    pub layer: String,
    pub samples_per_edge: u32,
    /// Row-major per-sample values.
    pub samples: Vec<f32>,
}

impl TerrainMask {
    /// Build a mask from raw row-major samples.
    pub fn from_samples(
        layer: impl Into<String>,
        samples_per_edge: u32,
        samples: Vec<f32>,
    ) -> Result<Self, TerrainDataError> {
        if samples_per_edge < 1 {
            return Err(TerrainDataError::TooSmall { samples_per_edge });
        }
        let expected = (samples_per_edge as usize) * (samples_per_edge as usize);
        if samples.len() != expected {
            return Err(TerrainDataError::SampleCountMismatch {
                expected,
                actual: samples.len(),
            });
        }
        Ok(Self {
            layer: layer.into(),
            samples_per_edge,
            samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_from_samples() {
        let mask = TerrainMask::from_samples("grass", 2, vec![0.0, 0.5, 1.0, 0.25]).unwrap();
        assert_eq!(mask.layer, "grass");
        assert_eq!(mask.samples.len(), 4);
    }

    #[test]
    fn rejects_mismatched_sample_count() {
        let err = TerrainMask::from_samples("rock", 2, vec![0.0; 3]).unwrap_err();
        assert_eq!(
            err,
            TerrainDataError::SampleCountMismatch {
                expected: 4,
                actual: 3,
            }
        );
    }
}
