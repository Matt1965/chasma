use core::fmt;

mod heightfield;
mod mask;
mod metadata;
mod source;

pub use heightfield::Heightfield;
pub use mask::TerrainMask;
pub use metadata::TerrainMetadata;
pub use source::{MaskSource, TerrainSource};

/// Errors produced when constructing authoritative terrain data from raw
/// samples (ADR-008).
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainDataError {
    /// Fewer samples per edge than the minimum required.
    TooSmall { samples_per_edge: u32 },
    /// Sample spacing was zero, negative, or non-finite.
    NonPositiveSpacing { spacing_meters: f32 },
    /// The provided sample buffer length did not match `samples_per_edge^2`.
    SampleCountMismatch { expected: usize, actual: usize },
}

impl fmt::Display for TerrainDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall { samples_per_edge } => write!(
                f,
                "terrain grid needs at least 2 samples per edge, got {samples_per_edge}"
            ),
            Self::NonPositiveSpacing { spacing_meters } => write!(
                f,
                "sample spacing must be positive and finite, got {spacing_meters}"
            ),
            Self::SampleCountMismatch { expected, actual } => {
                write!(f, "expected {expected} samples, got {actual}")
            }
        }
    }
}

impl std::error::Error for TerrainDataError {}
