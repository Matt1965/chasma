use core::fmt;

#[cfg(feature = "terrain-import")]
mod decode;
mod heightfield;
mod query;
#[cfg(feature = "terrain-import")]
mod gaea;
#[cfg(feature = "terrain-import")]
mod import;
mod mask;
mod metadata;

#[cfg(feature = "terrain-import")]
pub use decode::{DecodeError, decode_exr_heightfield};
pub use heightfield::Heightfield;
pub use query::{estimate_slope_degrees, ground_world_position, is_position_slope_walkable};
#[cfg(feature = "terrain-import")]
pub use gaea::{
    gaea_color_dir, gaea_height_dir, GaeaImportError, import_gaea_tile_directory,
    parse_gaea_export_filename, validate_gaea_tile_dimensions,
};
#[cfg(feature = "terrain-import")]
pub use import::{
    ImportError, SourceHeightfield, chunk_data_from_source_tile, expected_chunk_samples_per_edge,
    import_world, source_tile_samples_per_edge,
};
pub use mask::TerrainMask;
pub use metadata::TerrainMetadata;

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
