use core::fmt;

use super::{Heightfield, TerrainDataError};
use crate::world::{ChunkCoord, ChunkData, ChunkId, WorldConfig, WorldData};

/// A decoded source heightfield: a single row-major grid of `f32` heights with
/// known sample dimensions.
///
/// This is the boundary between *decoding* (e.g. reading an EXR, a later pass)
/// and *partitioning* (this module). It carries no spacing or world placement;
/// sample spacing and chunk size come from [`WorldConfig`] (ADR-008). Column
/// index advances along +X and row index along +Z, matching the chunk tile
/// layout (ADR-008 addendum: row 0 is minimum Z).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceHeightfield {
    width: u32,
    height: u32,
    samples: Vec<f32>,
}

impl SourceHeightfield {
    /// Build a source heightfield from raw row-major samples.
    ///
    /// All samples must be finite: heights are authoritative data (ADR-003), and
    /// NaN/infinite values would corrupt metadata and sampling.
    pub fn from_samples(width: u32, height: u32, samples: Vec<f32>) -> Result<Self, ImportError> {
        if width == 0 || height == 0 {
            return Err(ImportError::SourceDimensionZero { width, height });
        }
        let expected = width as usize * height as usize;
        if samples.len() != expected {
            return Err(ImportError::SourceSampleCountMismatch {
                expected,
                actual: samples.len(),
            });
        }
        if let Some(index) = samples.iter().position(|h| !h.is_finite()) {
            return Err(ImportError::NonFiniteSample { index });
        }
        Ok(Self {
            width,
            height,
            samples,
        })
    }

    fn sample(&self, col: u32, row: u32) -> f32 {
        self.samples[row as usize * self.width as usize + col as usize]
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// Errors produced while importing terrain data into [`WorldData`].
///
/// These cover the deterministic partitioning step. Decoder-specific failures
/// (file I/O, EXR decoding) are introduced alongside the decoder in a later
/// pass (ADR-009).
#[derive(Debug, Clone, PartialEq)]
pub enum ImportError {
    /// `WorldConfig` had a non-finite or non-positive chunk size or spacing.
    InvalidConfig {
        chunk_size_meters: f32,
        meters_per_sample: f32,
    },
    /// `units_per_meter` was not 1.0; the coordinate model fixes 1 unit = 1 m
    /// (ADR-001 addendum), which the importer relies on.
    UnsupportedUnitsPerMeter { units_per_meter: f32 },
    /// Chunk size is not an integer multiple of the sample spacing (ADR-008).
    ChunkSizeNotMultipleOfSampleSpacing {
        chunk_size_meters: f32,
        meters_per_sample: f32,
    },
    /// A source dimension was zero.
    SourceDimensionZero { width: u32, height: u32 },
    /// The source sample buffer length did not match `width * height`.
    SourceSampleCountMismatch { expected: usize, actual: usize },
    /// A source sample was NaN or infinite. Heights must be finite (ADR-003).
    NonFiniteSample { index: usize },
    /// The source is smaller than a single chunk tile.
    SourceTooSmall {
        source_width: u32,
        source_height: u32,
        required: u32,
    },
    /// Source dimensions do not partition into whole chunks with shared edges
    /// (`(dimension - 1)` must be a multiple of the per-chunk sample span).
    SourceNotChunkAligned {
        source_width: u32,
        source_height: u32,
        samples_per_chunk_edge: u32,
    },
    /// A per-chunk tile failed heightfield construction (should not occur if the
    /// alignment checks pass; propagated for safety).
    Heightfield(TerrainDataError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig {
                chunk_size_meters,
                meters_per_sample,
            } => write!(
                f,
                "invalid world config: chunk_size_meters={chunk_size_meters}, meters_per_sample={meters_per_sample}"
            ),
            Self::UnsupportedUnitsPerMeter { units_per_meter } => write!(
                f,
                "unsupported units_per_meter {units_per_meter}; the coordinate model fixes 1 unit = 1 meter (ADR-001)"
            ),
            Self::ChunkSizeNotMultipleOfSampleSpacing {
                chunk_size_meters,
                meters_per_sample,
            } => write!(
                f,
                "chunk size {chunk_size_meters} m is not an integer multiple of sample spacing {meters_per_sample} m"
            ),
            Self::SourceDimensionZero { width, height } => {
                write!(
                    f,
                    "source heightfield has a zero dimension: {width}x{height}"
                )
            }
            Self::SourceSampleCountMismatch { expected, actual } => {
                write!(f, "source expected {expected} samples, got {actual}")
            }
            Self::NonFiniteSample { index } => {
                write!(
                    f,
                    "source heightfield has a non-finite sample at index {index}"
                )
            }
            Self::SourceTooSmall {
                source_width,
                source_height,
                required,
            } => write!(
                f,
                "source {source_width}x{source_height} is smaller than one chunk tile ({required} samples per edge)"
            ),
            Self::SourceNotChunkAligned {
                source_width,
                source_height,
                samples_per_chunk_edge,
            } => write!(
                f,
                "source {source_width}x{source_height} does not partition into whole {samples_per_chunk_edge}-sample chunks"
            ),
            Self::Heightfield(err) => write!(f, "failed to build chunk heightfield: {err}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Deterministically import a decoded source heightfield into a populated
/// [`WorldData`] (ADR-008, ADR-009).
///
/// The source grid is partitioned into 256 m chunks (or whatever
/// `WorldConfig::chunk_size_meters` specifies) at `WorldConfig::meters_per_sample`
/// spacing. Each chunk stores `N + 1` samples per edge, sharing boundary samples
/// with its neighbors. Source sample (0, 0) maps to world origin and chunks are
/// emitted at non-negative coordinates (ADR-008 addendum). Identical inputs
/// always produce identical output.
///
/// Chunks created here have no mask layers; mask import is introduced with the
/// file decoder in a later pass.
pub fn import_world(
    source: &SourceHeightfield,
    config: &WorldConfig,
) -> Result<WorldData, ImportError> {
    // The coordinate model fixes 1 unit = 1 meter (ADR-001 addendum). The
    // importer relies on this when treating sample spacing (meters) as world
    // units during sampling, so reject configurations that would break it.
    if (config.units_per_meter - 1.0).abs() > 1e-6 {
        return Err(ImportError::UnsupportedUnitsPerMeter {
            units_per_meter: config.units_per_meter,
        });
    }

    let n = samples_per_chunk_span(config)?;
    let samples_per_edge = n + 1;

    if source.width < samples_per_edge || source.height < samples_per_edge {
        return Err(ImportError::SourceTooSmall {
            source_width: source.width,
            source_height: source.height,
            required: samples_per_edge,
        });
    }

    if (source.width - 1) % n != 0 || (source.height - 1) % n != 0 {
        return Err(ImportError::SourceNotChunkAligned {
            source_width: source.width,
            source_height: source.height,
            samples_per_chunk_edge: samples_per_edge,
        });
    }

    let chunks_x = (source.width - 1) / n;
    let chunks_z = (source.height - 1) / n;
    let spacing = config.meters_per_sample;

    let mut world = WorldData::new(config.chunk_layout());

    for cz in 0..chunks_z {
        for cx in 0..chunks_x {
            let tile = extract_tile(source, cx, cz, n);
            let heightfield = Heightfield::from_samples(samples_per_edge, spacing, tile)
                .map_err(ImportError::Heightfield)?;
            let data = ChunkData::new(heightfield, Vec::new());
            world.insert(ChunkId::new(ChunkCoord::new(cx as i32, cz as i32)), data);
        }
    }

    if chunks_x > 0 && chunks_z > 0 {
        world.set_authored_extent(crate::world::ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new((chunks_x - 1) as i32, (chunks_z - 1) as i32),
        });
    }

    Ok(world)
}

/// Samples per chunk edge (`N + 1`) expected for a single pre-chunked Gaea tile.
pub fn expected_chunk_samples_per_edge(config: &WorldConfig) -> Result<u32, ImportError> {
    Ok(samples_per_chunk_span(config)? + 1)
}

/// Samples per source-tile edge for non-overlapping Gaea exports (`N`, where the
/// runtime chunk uses `N + 1` shared-edge samples per ADR-008).
pub fn source_tile_samples_per_edge(config: &WorldConfig) -> Result<u32, ImportError> {
    samples_per_chunk_span(config)
}

/// Build [`ChunkData`] from a decoded tile that already covers exactly one chunk.
///
/// The source grid must be square with `expected_chunk_samples_per_edge(config)`
/// samples per side. This does not partition a larger heightfield; each Gaea EXR
/// tile maps 1:1 to one runtime chunk.
pub fn chunk_data_from_source_tile(
    source: &SourceHeightfield,
    config: &WorldConfig,
) -> Result<ChunkData, ImportError> {
    if (config.units_per_meter - 1.0).abs() > 1e-6 {
        return Err(ImportError::UnsupportedUnitsPerMeter {
            units_per_meter: config.units_per_meter,
        });
    }

    let samples_per_edge = expected_chunk_samples_per_edge(config)?;
    if source.width() != samples_per_edge || source.height() != samples_per_edge {
        return Err(ImportError::SourceNotChunkAligned {
            source_width: source.width(),
            source_height: source.height(),
            samples_per_chunk_edge: samples_per_edge,
        });
    }

    let heightfield = Heightfield::from_samples(
        samples_per_edge,
        config.meters_per_sample,
        source.samples().to_vec(),
    )
    .map_err(ImportError::Heightfield)?;

    Ok(ChunkData::new(heightfield, Vec::new()))
}

/// Compute `N`, the number of sample spans per chunk edge (so a tile has
/// `N + 1` samples per edge), validating that chunk size is an integer multiple
/// of the sample spacing (ADR-008).
///
/// `n * spacing` is the resulting tile span; requiring it to match the
/// configured chunk size keeps the heightfield tile domain consistent with the
/// coordinate layout. The tolerance is tight (a few ULP at 256 m) so genuinely
/// non-integer ratios are rejected rather than silently snapped to a near
/// integer.
fn samples_per_chunk_span(config: &WorldConfig) -> Result<u32, ImportError> {
    let chunk_size = config.chunk_size_meters;
    let spacing = config.meters_per_sample;

    if !chunk_size.is_finite() || chunk_size <= 0.0 || !spacing.is_finite() || spacing <= 0.0 {
        return Err(ImportError::InvalidConfig {
            chunk_size_meters: chunk_size,
            meters_per_sample: spacing,
        });
    }

    let ratio = chunk_size / spacing;
    let n = ratio.round();
    if n < 1.0 || (n * spacing - chunk_size).abs() > chunk_size * 1e-5 {
        return Err(ImportError::ChunkSizeNotMultipleOfSampleSpacing {
            chunk_size_meters: chunk_size,
            meters_per_sample: spacing,
        });
    }

    Ok(n as u32)
}

/// Extract the `(N + 1) x (N + 1)` sample tile for chunk `(cx, cz)`, sharing the
/// boundary samples with neighboring chunks (ADR-008).
fn extract_tile(source: &SourceHeightfield, cx: u32, cz: u32, n: u32) -> Vec<f32> {
    let samples_per_edge = n + 1;
    let base_col = cx * n;
    let base_row = cz * n;

    let mut tile = Vec::with_capacity((samples_per_edge * samples_per_edge) as usize);
    for r in 0..samples_per_edge {
        for c in 0..samples_per_edge {
            tile.push(source.sample(base_col + c, base_row + r));
        }
    }
    tile
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkExtent, ChunkId, WorldConfig};

    /// 2 m chunks at 1 m spacing -> N = 2, 3 samples per edge.
    fn config_small() -> WorldConfig {
        WorldConfig {
            chunk_size_meters: 2.0,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        }
    }

    /// Synthetic source where each sample encodes its grid position as
    /// `col + row * 100`, making provenance easy to assert.
    fn synthetic_source(width: u32, height: u32) -> SourceHeightfield {
        let mut samples = Vec::new();
        for row in 0..height {
            for col in 0..width {
                samples.push(col as f32 + row as f32 * 100.0);
            }
        }
        SourceHeightfield::from_samples(width, height, samples).unwrap()
    }

    #[test]
    fn partitions_into_expected_chunks_and_extent() {
        // 5x3 source, N=2 -> 2 chunks along X, 1 along Z.
        let world = import_world(&synthetic_source(5, 3), &config_small()).unwrap();
        assert_eq!(world.len(), 2);
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(0, 0))));
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(1, 0))));
        assert_eq!(
            world.extent(),
            Some(ChunkExtent {
                min: ChunkCoord::new(0, 0),
                max: ChunkCoord::new(1, 0),
            })
        );
    }

    #[test]
    fn shares_edge_samples_between_neighbors() {
        let world = import_world(&synthetic_source(5, 3), &config_small()).unwrap();
        let left_chunk = world.get(ChunkId::new(ChunkCoord::new(0, 0))).unwrap();
        let right_chunk = world.get(ChunkId::new(ChunkCoord::new(1, 0))).unwrap();

        let spe = left_chunk.heightfield.samples_per_edge();
        assert_eq!(spe, 3);

        // The right edge of chunk (0,0) must equal the left edge of chunk (1,0),
        // and both must equal source column 2 (the shared boundary).
        for row in 0..spe {
            let left_edge_of_right = right_chunk.heightfield.samples()[(row * spe) as usize];
            let right_edge_of_left =
                left_chunk.heightfield.samples()[(row * spe + (spe - 1)) as usize];
            assert_eq!(left_edge_of_right, right_edge_of_left);
            assert_eq!(right_edge_of_left, 2.0 + row as f32 * 100.0);
        }
    }

    #[test]
    fn derives_metadata_from_tile_samples() {
        let world = import_world(&synthetic_source(5, 3), &config_small()).unwrap();
        let chunk = world.get(ChunkId::new(ChunkCoord::new(0, 0))).unwrap();
        // Chunk (0,0) covers source cols 0..=2, rows 0..=2:
        // min = source(0,0) = 0, max = source(2,2) = 2 + 200 = 202.
        assert_eq!(chunk.metadata.height_min, 0.0);
        assert_eq!(chunk.metadata.height_max, 202.0);
    }

    #[test]
    fn import_is_deterministic() {
        let a = import_world(&synthetic_source(5, 3), &config_small()).unwrap();
        let b = import_world(&synthetic_source(5, 3), &config_small()).unwrap();
        assert_eq!(
            a.get(ChunkId::new(ChunkCoord::new(1, 0))),
            b.get(ChunkId::new(ChunkCoord::new(1, 0)))
        );
        assert_eq!(a.len(), b.len());
        assert_eq!(a.extent(), b.extent());
    }

    #[test]
    fn rejects_unaligned_source() {
        // width 4 -> (4 - 1) % 2 = 1, not chunk aligned.
        let err = import_world(&synthetic_source(4, 3), &config_small()).unwrap_err();
        assert!(matches!(err, ImportError::SourceNotChunkAligned { .. }));
    }

    #[test]
    fn rejects_source_smaller_than_one_chunk() {
        let err = import_world(&synthetic_source(2, 3), &config_small()).unwrap_err();
        assert!(matches!(err, ImportError::SourceTooSmall { .. }));
    }

    #[test]
    fn rejects_non_integer_sample_ratio() {
        let config = WorldConfig {
            chunk_size_meters: 2.5,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        };
        let err = import_world(&synthetic_source(5, 3), &config).unwrap_err();
        assert!(matches!(
            err,
            ImportError::ChunkSizeNotMultipleOfSampleSpacing { .. }
        ));
    }

    #[test]
    fn rejects_near_integer_chunk_size() {
        // 2.02 is close to an integer multiple of 1.0 but not exact; the tight
        // tolerance must reject it rather than snapping to N = 2.
        let config = WorldConfig {
            chunk_size_meters: 2.02,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        };
        let err = import_world(&synthetic_source(5, 3), &config).unwrap_err();
        assert!(matches!(
            err,
            ImportError::ChunkSizeNotMultipleOfSampleSpacing { .. }
        ));
    }

    #[test]
    fn rejects_non_unit_units_per_meter() {
        let config = WorldConfig {
            chunk_size_meters: 2.0,
            units_per_meter: 2.0,
            meters_per_sample: 1.0,
        };
        let err = import_world(&synthetic_source(5, 3), &config).unwrap_err();
        assert!(matches!(err, ImportError::UnsupportedUnitsPerMeter { .. }));
    }

    #[test]
    fn rejects_non_finite_source_samples() {
        let mut samples = vec![0.0f32; 9];
        samples[4] = f32::NAN;
        let err = SourceHeightfield::from_samples(3, 3, samples).unwrap_err();
        assert_eq!(err, ImportError::NonFiniteSample { index: 4 });
    }
}
