//! On-disk terrain field asset DTOs (ADR-101 TF1).

use serde::{Deserialize, Serialize};

use super::error::TerrainFieldLoadError;
use super::id::TerrainFieldId;
use super::tile::TerrainFieldTile;
use crate::world::ChunkCoord;

pub const TERRAIN_FIELD_MANIFEST_VERSION: u32 = 1;
pub const TERRAIN_FIELD_TILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldManifestConfig {
    pub chunk_size_meters: f32,
    pub sample_spacing_meters: f32,
    pub samples_per_edge: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldManifestEntry {
    pub field_id: String,
    pub tile_dir: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldManifest {
    pub version: u32,
    pub world_id: String,
    pub source_version: String,
    pub config: TerrainFieldManifestConfig,
    pub fields: Vec<TerrainFieldManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldTileFile {
    pub version: u32,
    pub field_id: String,
    pub x: i32,
    pub z: i32,
    pub samples_per_edge: u16,
    pub sample_spacing_meters: f32,
    pub source_version: String,
    pub samples: Vec<u16>,
    pub tile_revision: u64,
}

impl TerrainFieldTileFile {
    pub fn to_tile(&self) -> Result<TerrainFieldTile, TerrainFieldLoadError> {
        Ok(TerrainFieldTile {
            chunk: ChunkCoord::new(self.x, self.z),
            samples_per_edge: self.samples_per_edge,
            sample_spacing_meters: self.sample_spacing_meters,
            samples: self.samples.clone(),
            tile_revision: self.tile_revision,
            source_version: self.source_version.clone(),
        })
    }

    pub fn from_tile(field_id: &TerrainFieldId, tile: &TerrainFieldTile) -> Self {
        Self {
            version: TERRAIN_FIELD_TILE_VERSION,
            field_id: field_id.as_str().to_string(),
            x: tile.chunk.x,
            z: tile.chunk.z,
            samples_per_edge: tile.samples_per_edge,
            sample_spacing_meters: tile.sample_spacing_meters,
            source_version: tile.source_version.clone(),
            samples: tile.samples.clone(),
            tile_revision: tile.tile_revision,
        }
    }
}

pub fn decode_manifest(text: &str) -> Result<TerrainFieldManifest, TerrainFieldLoadError> {
    let manifest: TerrainFieldManifest =
        ron::from_str(text).map_err(|err| TerrainFieldLoadError::ManifestParse(err.to_string()))?;
    if manifest.version != TERRAIN_FIELD_MANIFEST_VERSION {
        return Err(TerrainFieldLoadError::ManifestVersionUnsupported {
            found: manifest.version,
            expected: TERRAIN_FIELD_MANIFEST_VERSION,
        });
    }
    Ok(manifest)
}

pub fn decode_tile(text: &str) -> Result<TerrainFieldTile, TerrainFieldLoadError> {
    let file: TerrainFieldTileFile =
        ron::from_str(text).map_err(|err| TerrainFieldLoadError::TileParse {
            path: String::new(),
            message: err.to_string(),
        })?;
    if file.version != TERRAIN_FIELD_TILE_VERSION {
        return Err(TerrainFieldLoadError::TileParse {
            path: String::new(),
            message: format!(
                "unsupported tile version {} expected {}",
                file.version, TERRAIN_FIELD_TILE_VERSION
            ),
        });
    }
    file.to_tile()
}

pub fn tile_path_for_chunk(dir: &str, chunk: ChunkCoord) -> String {
    format!("{dir}/{x}_{z}.ron", x = chunk.x, z = chunk.z)
}
