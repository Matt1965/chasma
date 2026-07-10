use core::fmt;

#[cfg(feature = "terrain-import")]
mod decode;
mod contract;
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
pub use contract::{
    validate_heightfield_against_config, CHUNK_SPAN_RELATIVE_TOLERANCE,
    SPACING_TOLERANCE_METERS,
};
pub use heightfield::Heightfield;
pub use query::{
    classify_slope_walkability, estimate_slope_degrees, ground_world_position,
    is_position_slope_walkable, slope_at, try_ground_world_position,
    try_sample_height_at_position, SlopeWalkability,
};
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
/// samples (ADR-008, REVIEW-B6).
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainDataError {
    /// Fewer samples per edge than the minimum required.
    TooSmall { samples_per_edge: u32 },
    /// Sample spacing was zero, negative, or non-finite.
    NonPositiveSpacing { spacing_meters: f32 },
    /// The provided sample buffer length did not match `samples_per_edge^2`.
    SampleCountMismatch { expected: usize, actual: usize },
    /// A height sample was NaN or infinite.
    NonFiniteHeightSample { index: usize },
    /// Grid dimensions do not match the world terrain contract.
    InvalidDimensions {
        expected_samples_per_edge: u32,
        found_samples_per_edge: u32,
    },
    /// Stored spacing does not match [`crate::world::WorldConfig::meters_per_sample`].
    SpacingMismatch {
        expected_meters: f32,
        found_meters: f32,
    },
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
            Self::NonFiniteHeightSample { index } => {
                write!(f, "height sample at index {index} is not finite")
            }
            Self::InvalidDimensions {
                expected_samples_per_edge,
                found_samples_per_edge,
            } => write!(
                f,
                "expected {expected_samples_per_edge} samples per edge, found {found_samples_per_edge}"
            ),
            Self::SpacingMismatch {
                expected_meters,
                found_meters,
            } => write!(
                f,
                "expected spacing {expected_meters} m, found {found_meters} m"
            ),
        }
    }
}

impl std::error::Error for TerrainDataError {}

/// Structured failure for authoritative terrain queries (REVIEW-B4, ADR-067).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainQueryError {
    /// Owning chunk heightfield is not resident in [`crate::world::WorldData`].
    ChunkNotResident,
    /// Chunk-local XZ lies outside the heightfield domain.
    InvalidTerrainCoordinate,
    /// Slope could not be estimated at the position (missing chunk or domain).
    SlopeUnavailable,
}

impl fmt::Display for TerrainQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChunkNotResident => write!(f, "terrain chunk not resident"),
            Self::InvalidTerrainCoordinate => write!(f, "terrain coordinate outside heightfield domain"),
            Self::SlopeUnavailable => write!(f, "terrain slope unavailable"),
        }
    }
}

impl std::error::Error for TerrainQueryError {}
