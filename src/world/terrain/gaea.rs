//! Offline Gaea per-tile EXR import (ADR-009, ADR-011).
//!
//! Each `Export_y{z}_x{x}.exr` file from Gaea becomes exactly one runtime chunk.
//! Gaea non-overlap exports are `N x N` per tile; runtime chunks remain `N+1`
//! with shared boundary samples (ADR-008). This module stitches tile edges from
//! neighbors (or duplicates the local boundary at the outer world edge).

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::terrain::write::write_world;
use crate::terrain::TerrainAssetError;
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, Heightfield, WorldConfig, WorldData,
};

use super::decode::{decode_exr_heightfield, DecodeError};
use super::import::{
    chunk_data_from_source_tile, expected_chunk_samples_per_edge, source_tile_samples_per_edge,
    ImportError, SourceHeightfield,
};

/// How a Gaea source tile maps to the runtime chunk grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaeaTileLayout {
    /// Tile already includes shared-edge samples (`N+1` per side).
    SharedEdge,
    /// Non-overlapping Gaea export (`N` per side); edges are stitched at import.
    NonOverlap,
}

/// Errors produced while importing a directory of Gaea EXR tiles.
#[derive(Debug, Clone, PartialEq)]
pub enum GaeaImportError {
    Io { path: String, message: String },
    InvalidFilename { filename: String },
    NoTilesFound { directory: String },
    DuplicateChunk { x: i32, z: i32 },
    Decode { path: String, error: DecodeError },
    TileDimensionMismatch {
        path: String,
        actual_width: u32,
        actual_height: u32,
        expected_non_overlap_edge: u32,
        expected_shared_edge: u32,
        chunk_size_meters: f32,
        meters_per_sample: f32,
    },
    Config(ImportError),
    Chunk(ImportError),
    Write(TerrainAssetError),
}

impl fmt::Display for GaeaImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "io error for {path}: {message}"),
            Self::InvalidFilename { filename } => {
                write!(
                    f,
                    "filename {filename:?} does not match Export_y<z>_x<x>.exr"
                )
            }
            Self::NoTilesFound { directory } => {
                write!(f, "no .exr tiles found in {directory}")
            }
            Self::DuplicateChunk { x, z } => {
                write!(f, "duplicate chunk ({x}, {z}) in import directory")
            }
            Self::Decode { path, error } => write!(f, "failed to decode {path}: {error}"),
            Self::TileDimensionMismatch {
                path,
                actual_width,
                actual_height,
                expected_non_overlap_edge,
                expected_shared_edge,
                chunk_size_meters,
                meters_per_sample,
            } => write!(
                f,
                "tile {path} is {actual_width}x{actual_height}; expected square \
                 {expected_non_overlap_edge}x{expected_non_overlap_edge} (Gaea non-overlap) \
                 or {expected_shared_edge}x{expected_shared_edge} (shared-edge) for \
                 chunk_size_meters={chunk_size_meters} and meters_per_sample={meters_per_sample}"
            ),
            Self::Config(err) => write!(f, "invalid world config for Gaea import: {err}"),
            Self::Chunk(err) => write!(f, "failed to build chunk from tile: {err}"),
            Self::Write(err) => write!(f, "failed to write runtime assets: {err}"),
        }
    }
}

impl std::error::Error for GaeaImportError {}

/// Parse `Export_y{z}_x{x}.exr` into chunk coordinates.
///
/// `x` maps to [`ChunkCoord::x`]; `y` maps to [`ChunkCoord::z`].
pub fn parse_gaea_export_filename(filename: &str) -> Result<(i32, i32), GaeaImportError> {
    let name = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    const PREFIX: &str = "Export_y";
    const SUFFIX: &str = ".exr";

    if !name.starts_with(PREFIX) || !name.ends_with(SUFFIX) {
        return Err(GaeaImportError::InvalidFilename {
            filename: name.to_string(),
        });
    }

    let inner = &name[PREFIX.len()..name.len() - SUFFIX.len()];
    let (z_str, x_str) = inner.split_once("_x").ok_or(GaeaImportError::InvalidFilename {
        filename: name.to_string(),
    })?;

    let z: i32 = z_str.parse().map_err(|_| GaeaImportError::InvalidFilename {
        filename: name.to_string(),
    })?;
    let x: i32 = x_str.parse().map_err(|_| GaeaImportError::InvalidFilename {
        filename: name.to_string(),
    })?;

    Ok((x, z))
}

/// Classify a decoded tile's sample grid against [`WorldConfig`].
pub fn classify_gaea_tile_layout(
    actual_width: u32,
    actual_height: u32,
    config: &WorldConfig,
) -> Result<GaeaTileLayout, ImportError> {
    let shared_edge = expected_chunk_samples_per_edge(config)?;
    let non_overlap = source_tile_samples_per_edge(config)?;

    if actual_width == shared_edge && actual_height == shared_edge {
        return Ok(GaeaTileLayout::SharedEdge);
    }
    if actual_width == non_overlap && actual_height == non_overlap {
        return Ok(GaeaTileLayout::NonOverlap);
    }

    Err(ImportError::SourceNotChunkAligned {
        source_width: actual_width,
        source_height: actual_height,
        samples_per_chunk_edge: shared_edge,
    })
}

/// Validate that a decoded tile matches the expected single-chunk sample grid.
pub fn validate_gaea_tile_dimensions(
    actual_width: u32,
    actual_height: u32,
    config: &WorldConfig,
    path: &Path,
) -> Result<GaeaTileLayout, GaeaImportError> {
    match classify_gaea_tile_layout(actual_width, actual_height, config) {
        Ok(layout) => Ok(layout),
        Err(_) => {
            let expected_shared_edge =
                expected_chunk_samples_per_edge(config).map_err(GaeaImportError::Config)?;
            let expected_non_overlap =
                source_tile_samples_per_edge(config).map_err(GaeaImportError::Config)?;
            Err(GaeaImportError::TileDimensionMismatch {
                path: path.display().to_string(),
                actual_width,
                actual_height,
                expected_non_overlap_edge: expected_non_overlap,
                expected_shared_edge,
                chunk_size_meters: config.chunk_size_meters,
                meters_per_sample: config.meters_per_sample,
            })
        }
    }
}

fn sample_at(tile: &SourceHeightfield, col: u32, row: u32) -> f32 {
    let w = tile.width() as usize;
    tile.samples()[row as usize * w + col as usize]
}

/// Expand a non-overlapping `N x N` Gaea tile into runtime `N+1` row-major samples.
///
/// Interior samples come from the tile. The `+X` edge uses column 0 of `(x+1, z)`;
/// the `+Z` edge uses row 0 of `(x, z+1)`; the corner uses `(0, 0)` of `(x+1, z+1)`.
/// Missing neighbors at the outer world boundary duplicate the local last row,
/// column, or corner.
pub fn expand_non_overlap_tile(
    x: i32,
    z: i32,
    tile: &SourceHeightfield,
    neighbors: &HashMap<(i32, i32), &SourceHeightfield>,
    source_edge: u32,
) -> Vec<f32> {
    let runtime_edge = source_edge + 1;
    let last = source_edge - 1;
    let mut out = Vec::with_capacity((runtime_edge * runtime_edge) as usize);

    for row in 0..runtime_edge {
        for col in 0..runtime_edge {
            let height = if col < source_edge && row < source_edge {
                sample_at(tile, col, row)
            } else if col == source_edge && row < source_edge {
                neighbors
                    .get(&(x + 1, z))
                    .map(|n| sample_at(n, 0, row))
                    .unwrap_or_else(|| sample_at(tile, last, row))
            } else if row == source_edge && col < source_edge {
                neighbors
                    .get(&(x, z + 1))
                    .map(|n| sample_at(n, col, 0))
                    .unwrap_or_else(|| sample_at(tile, col, last))
            } else {
                neighbors
                    .get(&(x + 1, z + 1))
                    .map(|n| sample_at(n, 0, 0))
                    .unwrap_or_else(|| sample_at(tile, last, last))
            };
            out.push(height);
        }
    }

    let has_east = neighbors.contains_key(&(x + 1, z));
    let has_north = neighbors.contains_key(&(x, z + 1));
    let _ = (has_east, has_north);
    repair_non_overlap_edge_slopes(&mut out, runtime_edge as usize);

    out
}

/// Linearly ramp interior samples toward the stitched +X / +Z boundary.
fn repair_non_overlap_edge_slopes(heights: &mut [f32], spe: usize) {
    const EDGE_RAMP_SAMPLES: usize = 1;
    if spe < 2 {
        return;
    }
    let last = spe - 1;
    let ramp_len = last.min(EDGE_RAMP_SAMPLES);
    if ramp_len < 2 {
        return;
    }
    let ramp_start = last - ramp_len;

    for row in 0..last {
        let base = row * spe;
        let hi = heights[base + ramp_start];
        let hb = heights[base + last];
        for step in 1..ramp_len {
            let t = step as f32 / ramp_len as f32;
            heights[base + ramp_start + step] = hi + (hb - hi) * t;
        }
    }

    for col in 0..ramp_start {
        let hi = heights[ramp_start * spe + col];
        let hb = heights[last * spe + col];
        for step in 1..ramp_len {
            let t = step as f32 / ramp_len as f32;
            heights[(ramp_start + step) * spe + col] = hi + (hb - hi) * t;
        }
    }
}

fn chunk_from_non_overlap_tile(
    x: i32,
    z: i32,
    tile: &SourceHeightfield,
    neighbors: &HashMap<(i32, i32), &SourceHeightfield>,
    config: &WorldConfig,
) -> Result<ChunkData, ImportError> {
    if (config.units_per_meter - 1.0).abs() > 1e-6 {
        return Err(ImportError::UnsupportedUnitsPerMeter {
            units_per_meter: config.units_per_meter,
        });
    }

    let source_edge = source_tile_samples_per_edge(config)?;
    let runtime_edge = source_edge + 1;
    let samples = expand_non_overlap_tile(x, z, tile, neighbors, source_edge);
    let heightfield = Heightfield::from_samples(
        runtime_edge,
        config.meters_per_sample,
        samples,
    )
    .map_err(ImportError::Heightfield)?;

    Ok(ChunkData::new(heightfield, Vec::new()))
}

fn io_err(path: &Path, err: std::io::Error) -> GaeaImportError {
    GaeaImportError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    }
}

struct LoadedGaeaTile {
    x: i32,
    z: i32,
    source: SourceHeightfield,
    layout: GaeaTileLayout,
}

/// Import every Gaea `Export_y{z}_x{x}.exr` tile in `input_dir` into runtime assets.
///
/// Each EXR becomes one [`ChunkData`] at the parsed [`ChunkId`], then
/// `output_world_dir/manifest.ron` and `output_world_dir/chunks/<x>_<z>.ron`
/// are written (ADR-011). Returns the number of chunks imported.
pub fn import_gaea_tile_directory(
    input_dir: &Path,
    output_world_dir: &Path,
    config: &WorldConfig,
) -> Result<usize, GaeaImportError> {
    let mut paths: Vec<(PathBuf, i32, i32)> = Vec::new();
    for entry in fs::read_dir(input_dir).map_err(|err| io_err(input_dir, err))? {
        let entry = entry.map_err(|err| io_err(input_dir, err))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("exr") {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| GaeaImportError::InvalidFilename {
                filename: path.display().to_string(),
            })?;
        let (x, z) = parse_gaea_export_filename(filename)?;
        paths.push((path, x, z));
    }

    if paths.is_empty() {
        return Err(GaeaImportError::NoTilesFound {
            directory: input_dir.display().to_string(),
        });
    }

    paths.sort_by_key(|(_, x, z)| (*z, *x));

    let mut loaded: Vec<LoadedGaeaTile> = Vec::with_capacity(paths.len());
    for (path, x, z) in paths {
        if loaded.iter().any(|t| t.x == x && t.z == z) {
            return Err(GaeaImportError::DuplicateChunk { x, z });
        }

        let source = decode_exr_heightfield(&path).map_err(|error| GaeaImportError::Decode {
            path: path.display().to_string(),
            error,
        })?;

        let layout = validate_gaea_tile_dimensions(
            source.width(),
            source.height(),
            config,
            &path,
        )?;

        loaded.push(LoadedGaeaTile {
            x,
            z,
            source,
            layout,
        });
    }

    let neighbors: HashMap<(i32, i32), &SourceHeightfield> = loaded
        .iter()
        .map(|t| ((t.x, t.z), &t.source))
        .collect();

    let mut world = WorldData::new(config.chunk_layout());

    for tile in &loaded {
        let chunk_id = ChunkId::new(ChunkCoord::new(tile.x, tile.z));
        let chunk = match tile.layout {
            GaeaTileLayout::SharedEdge => {
                chunk_data_from_source_tile(&tile.source, config).map_err(GaeaImportError::Chunk)?
            }
            GaeaTileLayout::NonOverlap => chunk_from_non_overlap_tile(
                tile.x,
                tile.z,
                &tile.source,
                &neighbors,
                config,
            )
            .map_err(GaeaImportError::Chunk)?,
        };
        world.insert(chunk_id, chunk);
    }

    let count = world.len();
    write_world(output_world_dir, config, &world).map_err(GaeaImportError::Write)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::load::load_world_from_manifest;
    use crate::world::terrain::SourceHeightfield;

    fn tile_from_grid(width: u32, heights: &[f32]) -> SourceHeightfield {
        SourceHeightfield::from_samples(width, width, heights.to_vec()).unwrap()
    }

    #[test]
    fn parses_gaea_export_filenames() {
        assert_eq!(
            parse_gaea_export_filename("Export_y0_x0.exr").unwrap(),
            (0, 0)
        );
        assert_eq!(
            parse_gaea_export_filename("Export_y1_x0.exr").unwrap(),
            (0, 1)
        );
        assert_eq!(
            parse_gaea_export_filename("Export_y0_x1.exr").unwrap(),
            (1, 0)
        );
        assert_eq!(
            parse_gaea_export_filename("Export_y1_x1.exr").unwrap(),
            (1, 1)
        );
    }

    #[test]
    fn rejects_invalid_gaea_filenames() {
        assert!(matches!(
            parse_gaea_export_filename("terrain_0_0.exr"),
            Err(GaeaImportError::InvalidFilename { .. })
        ));
    }

    #[test]
    fn accepts_shared_edge_and_non_overlap_dimensions() {
        let config = WorldConfig::default();
        let shared = expected_chunk_samples_per_edge(&config).unwrap();
        let non_overlap = source_tile_samples_per_edge(&config).unwrap();

        assert_eq!(
            validate_gaea_tile_dimensions(shared, shared, &config, Path::new("t.exr")).unwrap(),
            GaeaTileLayout::SharedEdge
        );
        assert_eq!(
            validate_gaea_tile_dimensions(non_overlap, non_overlap, &config, Path::new("t.exr"))
                .unwrap(),
            GaeaTileLayout::NonOverlap
        );
    }

    #[test]
    fn rejects_unsupported_tile_dimensions() {
        let config = WorldConfig::default();
        let err = validate_gaea_tile_dimensions(100, 100, &config, Path::new("bad.exr"))
            .unwrap_err();
        assert!(matches!(
            err,
            GaeaImportError::TileDimensionMismatch { actual_width: 100, .. }
        ));
    }

    #[test]
    fn rejects_non_square_tile_dimensions() {
        let config = WorldConfig::default();
        let non_overlap = source_tile_samples_per_edge(&config).unwrap();
        let err = validate_gaea_tile_dimensions(
            non_overlap,
            non_overlap - 1,
            &config,
            Path::new("bad.exr"),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            GaeaImportError::TileDimensionMismatch { .. }
        ));
    }

    #[test]
    fn stitches_2x2_non_overlap_tiles_to_shared_runtime_chunks() {
        let config = WorldConfig {
            chunk_size_meters: 2.0,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        };
        let n = source_tile_samples_per_edge(&config).unwrap();
        assert_eq!(n, 2);

        // Unique heights per cell: col + row * 10 within each tile.
        let t00 = tile_from_grid(n, &[0.0, 1.0, 10.0, 11.0]);
        let t10 = tile_from_grid(n, &[100.0, 101.0, 110.0, 111.0]);
        let t01 = tile_from_grid(n, &[1000.0, 1001.0, 1010.0, 1011.0]);
        let t11 = tile_from_grid(n, &[10000.0, 10001.0, 10010.0, 10011.0]);

        let neighbors = HashMap::from([
            ((0, 0), &t00),
            ((1, 0), &t10),
            ((0, 1), &t01),
            ((1, 1), &t11),
        ]);

        let expanded = expand_non_overlap_tile(0, 0, &t00, &neighbors, n);
        let runtime_edge = n + 1;
        assert_eq!(expanded.len(), (runtime_edge * runtime_edge) as usize);

        let at = |col: u32, row: u32| {
            expanded[row as usize * runtime_edge as usize + col as usize]
        };

        assert_eq!(at(0, 0), 0.0);
        assert_eq!(at(1, 0), 50.0);
        assert_eq!(at(0, 1), 10.0);
        assert_eq!(at(1, 1), 60.0);
        assert_eq!(at(2, 0), 100.0);
        assert_eq!(at(2, 1), 110.0);
        assert_eq!(at(0, 2), 1000.0);
        assert_eq!(at(1, 2), 1001.0);
        assert_eq!(at(2, 2), 10000.0);

        let c00 = chunk_from_non_overlap_tile(0, 0, &t00, &neighbors, &config).unwrap();
        let c10 = chunk_from_non_overlap_tile(1, 0, &t10, &neighbors, &config).unwrap();
        let c01 = chunk_from_non_overlap_tile(0, 1, &t01, &neighbors, &config).unwrap();

        assert_eq!(c00.heightfield.samples_per_edge(), runtime_edge);
        assert_eq!(
            c00.heightfield.sample(2.0 * config.meters_per_sample, 0.0),
            c10.heightfield.sample(0.0, 0.0)
        );
        assert_eq!(
            c00.heightfield.sample(0.0, 2.0 * config.meters_per_sample),
            c01.heightfield.sample(0.0, 0.0)
        );
    }

    #[test]
    fn duplicates_boundary_when_neighbor_missing() {
        let config = WorldConfig {
            chunk_size_meters: 2.0,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        };
        let n = source_tile_samples_per_edge(&config).unwrap();
        let tile = tile_from_grid(n, &[0.0, 1.0, 10.0, 11.0]);
        let neighbors = HashMap::from([((0, 0), &tile)]);

        let expanded = expand_non_overlap_tile(0, 0, &tile, &neighbors, n);
        let runtime_edge = n + 1;
        let at = |col: u32, row: u32| {
            expanded[row as usize * runtime_edge as usize + col as usize]
        };

        assert_eq!(at(2, 0), 1.0);
        assert_eq!(at(2, 1), 11.0);
        assert_eq!(at(0, 2), 10.0);
        assert_eq!(at(1, 2), 11.0);
        assert_eq!(at(2, 2), 11.0);
    }

    #[test]
    fn imports_synthetic_shared_edge_gaea_tile_directory() {
        use exr::prelude::write_rgb_file;
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let config = WorldConfig {
            chunk_size_meters: 2.0,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        };
        let spe = expected_chunk_samples_per_edge(&config).unwrap();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let input = std::env::temp_dir().join(format!("chasma_gaea_in_{}_{n}", std::process::id()));
        let output =
            std::env::temp_dir().join(format!("chasma_gaea_out_{}_{n}", std::process::id()));
        fs::create_dir_all(&input).unwrap();

        for (x, z) in [(0, 0), (1, 0)] {
            let path = input.join(format!("Export_y{z}_x{x}.exr"));
            write_rgb_file(&path, spe as usize, spe as usize, |cx, cy| {
                let h = (cx + cy) as f32;
                (h, h, h)
            })
            .unwrap();
        }

        let count = import_gaea_tile_directory(&input, &output, &config).unwrap();
        assert_eq!(count, 2);

        let mut world = WorldData::new(config.chunk_layout());
        load_world_from_manifest(&output.join("manifest.ron"), &config, &mut world).unwrap();
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(0, 0))));

        fs::remove_dir_all(&input).ok();
        fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn imports_synthetic_non_overlap_2x2_gaea_directory() {
        use exr::prelude::write_rgb_file;
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let config = WorldConfig {
            chunk_size_meters: 2.0,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        };
        let n = source_tile_samples_per_edge(&config).unwrap();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let input = std::env::temp_dir().join(format!("chasma_gaea_noin_{id}"));
        let output = std::env::temp_dir().join(format!("chasma_gaea_noout_{id}"));
        fs::create_dir_all(&input).unwrap();

        let heights = |x: i32, z: i32, col: usize, row: usize| {
            (x * 1000 + z * 100 + col as i32 + row as i32 * 10) as f32
        };

        for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let path = input.join(format!("Export_y{z}_x{x}.exr"));
            write_rgb_file(&path, n as usize, n as usize, |col, row| {
                let h = heights(x, z, col, row);
                (h, h, h)
            })
            .unwrap();
        }

        let count = import_gaea_tile_directory(&input, &output, &config).unwrap();
        assert_eq!(count, 4);

        let mut world = WorldData::new(config.chunk_layout());
        load_world_from_manifest(&output.join("manifest.ron"), &config, &mut world).unwrap();

        let c00 = world
            .get(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap();
        let c10 = world
            .get(ChunkId::new(ChunkCoord::new(1, 0)))
            .unwrap();
        let c01 = world
            .get(ChunkId::new(ChunkCoord::new(0, 1)))
            .unwrap();

        assert_eq!(
            c00.heightfield.sample(2.0, 0.0),
            c10.heightfield.sample(0.0, 0.0)
        );
        assert_eq!(
            c00.heightfield.sample(0.0, 2.0),
            c01.heightfield.sample(0.0, 0.0)
        );

        fs::remove_dir_all(&input).ok();
        fs::remove_dir_all(&output).ok();
    }

    /// Imports `source_data/test` into `assets/worlds/main`.
    ///
    /// Run manually:
    /// `cargo test --features terrain-import import_source_data_test -- --ignored`
    #[test]
    #[ignore = "manual: imports source_data/test into assets/worlds/main"]
    fn import_source_data_test() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let input = root.join("source_data/test");
        let output = root.join("assets/worlds/main");
        let expected = fs::read_dir(&input)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    == Some("exr")
            })
            .count();
        assert!(expected > 0, "source_data/test must contain at least one .exr tile");
        let count = import_gaea_tile_directory(&input, &output, &WorldConfig::default()).unwrap();
        assert_eq!(count, expected, "one runtime chunk per source EXR");
    }
}
