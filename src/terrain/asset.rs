//! Pre-chunked terrain asset format types (ADR-011).
//!
//! These are plain, serde-serializable data-transfer types for the on-disk
//! format. They live in the Terrain Runtime Layer and use primitive fields so
//! that `serde` does not leak into the World Data Layer types (ADR-007 defers
//! serde on world types until persistence is real). Decoding converts these DTOs
//! into authoritative [`crate::world::ChunkData`]; encoding is the reverse, in the
//! offline writer.
//!
//! Phase 2A stores **one self-contained chunk file per chunk** and a manifest
//! mapping each chunk to its file path. There are no region containers, indexes,
//! or mask payloads (ADR-011, ADR-012).

use core::fmt;

use serde::{Deserialize, Serialize};

use super::albedo::AlbedoGridError;
use crate::world::TerrainDataError;

/// Version of the per-chunk file payload. Bumped if the encoding changes.
pub const CHUNK_FORMAT_VERSION: u32 = 1;
/// Version of the manifest payload.
pub const MANIFEST_FORMAT_VERSION: u32 = 1;
/// Version of the optional albedo sidecar RON payload (ADR-011 addendum).
pub const ALBEDO_FORMAT_VERSION: u32 = 1;

/// The `WorldConfig` snapshot embedded in a manifest, used to validate that
/// assets were produced for the same spatial layout the runtime is using.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManifestConfig {
    pub chunk_size_meters: f32,
    pub units_per_meter: f32,
    pub meters_per_sample: f32,
}

/// One chunk entry in the manifest: its coordinate identity and the path to its
/// chunk file, relative to the manifest's own directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestChunk {
    pub x: i32,
    pub z: i32,
    pub path: String,
    /// Optional albedo sidecar path, relative to the manifest directory (ADR-011).
    #[serde(default)]
    pub albedo_path: Option<String>,
}

impl ManifestChunk {
    pub fn at(x: i32, z: i32, path: impl Into<String>) -> Self {
        Self {
            x,
            z,
            path: path.into(),
            albedo_path: None,
        }
    }

    pub fn with_albedo(mut self, albedo_path: impl Into<String>) -> Self {
        self.albedo_path = Some(albedo_path.into());
        self
    }
}

/// The world manifest (`assets/worlds/<name>/manifest.ron`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub config: ManifestConfig,
    pub chunks: Vec<ManifestChunk>,
}

/// A single self-contained chunk file payload (ADR-008, ADR-011).
///
/// Carries the authoritative heightfield tile plus the derived height range.
/// Masks are intentionally absent in Phase 2A (ADR-009 Phase 1 Cleanup).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkFile {
    pub version: u32,
    pub x: i32,
    pub z: i32,
    pub samples_per_edge: u32,
    pub spacing_meters: f32,
    /// Row-major `samples_per_edge * samples_per_edge` heights.
    pub samples: Vec<f32>,
    /// Derived height range (recomputed and validated on decode).
    pub height_min: f32,
    pub height_max: f32,
}

/// Optional per-chunk albedo sidecar payload (`*.albedo.ron`, ADR-011 addendum).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbedoFile {
    pub version: u32,
    pub samples_per_edge: u32,
    /// Row-major linear RGB triples.
    pub samples: Vec<[f32; 3]>,
}

/// Errors produced while decoding, loading, or writing pre-chunked terrain
/// assets.
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainAssetError {
    /// RON (de)serialization failed.
    Ron(String),
    /// A filesystem operation failed.
    Io { path: String, message: String },
    /// An unsupported format version was encountered.
    UnsupportedVersion { found: u32, expected: u32 },
    /// The heightfield tile failed authoritative construction (ADR-008).
    Heightfield(TerrainDataError),
    /// The stored height range did not match the range recomputed from samples.
    MetadataMismatch {
        x: i32,
        z: i32,
        stored_min: f32,
        stored_max: f32,
        computed_min: f32,
        computed_max: f32,
    },
    /// The manifest's config snapshot did not match the runtime `WorldConfig`.
    ConfigMismatch {
        manifest: ManifestConfig,
        runtime: ManifestConfig,
    },
    /// A manifest entry's coordinates did not match its chunk file payload.
    ChunkCoordMismatch {
        manifest_x: i32,
        manifest_z: i32,
        file_x: i32,
        file_z: i32,
    },
    /// A decoded chunk's heightfield span did not match `WorldConfig`.
    ChunkSizeMismatch {
        x: i32,
        z: i32,
        expected_meters: f32,
        found_meters: f32,
    },
    /// Albedo grid construction failed.
    AlbedoGrid(AlbedoGridError),
    /// Albedo sidecar decode failed.
    AlbedoDecode { path: String, message: String },
    /// Albedo grid dimensions did not match the height chunk grid.
    AlbedoDimensionMismatch {
        path: String,
        width: usize,
        height: usize,
        expected_samples_per_edge: usize,
    },
    /// Albedo sidecar file extension is not supported in this build.
    AlbedoUnsupportedFormat { path: String, extension: String },
}

impl fmt::Display for TerrainAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ron(msg) => write!(f, "RON error: {msg}"),
            Self::Io { path, message } => write!(f, "io error for {path}: {message}"),
            Self::UnsupportedVersion { found, expected } => {
                write!(f, "unsupported format version {found}, expected {expected}")
            }
            Self::Heightfield(err) => write!(f, "invalid chunk heightfield: {err}"),
            Self::MetadataMismatch {
                x,
                z,
                stored_min,
                stored_max,
                computed_min,
                computed_max,
            } => write!(
                f,
                "chunk ({x}, {z}) stored height range [{stored_min}, {stored_max}] does not match computed [{computed_min}, {computed_max}]"
            ),
            Self::ConfigMismatch { manifest, runtime } => write!(
                f,
                "manifest config {manifest:?} does not match runtime config {runtime:?}"
            ),
            Self::ChunkCoordMismatch {
                manifest_x,
                manifest_z,
                file_x,
                file_z,
            } => write!(
                f,
                "manifest chunk ({manifest_x}, {manifest_z}) does not match file ({file_x}, {file_z})"
            ),
            Self::ChunkSizeMismatch {
                x,
                z,
                expected_meters,
                found_meters,
            } => write!(
                f,
                "chunk ({x}, {z}) span {found_meters} m does not match expected {expected_meters} m"
            ),
            Self::AlbedoGrid(err) => write!(f, "invalid albedo grid: {err}"),
            Self::AlbedoDecode { path, message } => {
                write!(f, "failed to decode albedo sidecar {path}: {message}")
            }
            Self::AlbedoDimensionMismatch {
                path,
                width,
                height,
                expected_samples_per_edge,
            } => write!(
                f,
                "albedo sidecar {path} is {width}x{height}, expected square {expected_samples_per_edge}x{expected_samples_per_edge}"
            ),
            Self::AlbedoUnsupportedFormat { path, extension } => write!(
                f,
                "unsupported albedo sidecar format {extension:?} for {path}"
            ),
        }
    }
}

impl std::error::Error for TerrainAssetError {}
