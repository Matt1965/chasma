//! Heightfield and biome dependencies for offline generation (ADR-102).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::super::source_error::TerrainFieldSourceError;
use crate::terrain::catalog::TerrainWorldCatalog;
use crate::terrain::decode::decode_chunk;
use crate::world::biome::BiomeMask;
use crate::world::estimate_slope_degrees;
use crate::world::{ChunkCoord, ChunkExtent, ChunkLayout, Heightfield, WorldConfig};

/// Samples authoritative terrain height at global XZ from packaged heightfields.
#[derive(Debug, Clone)]
pub struct HeightfieldDependency {
    layout: ChunkLayout,
    extent: ChunkExtent,
    tiles: HashMap<ChunkCoord, Heightfield>,
}

impl HeightfieldDependency {
    pub fn load_from_terrain_catalog(
        catalog: &TerrainWorldCatalog,
        extent: ChunkExtent,
        config: &WorldConfig,
    ) -> Result<Self, TerrainFieldSourceError> {
        let layout = config.chunk_layout();
        let mut tiles = HashMap::new();
        for z in extent.min.z..=extent.max.z {
            for x in extent.min.x..=extent.max.x {
                let coord = ChunkCoord::new(x, z);
                let Some(path) = catalog.chunk_path(coord) else {
                    return Err(TerrainFieldSourceError::GeneratorDependencyMissing(
                        format!("heightfield chunk ({x}, {z})"),
                    ));
                };
                let text = std::fs::read_to_string(path).map_err(|err| {
                    TerrainFieldSourceError::GeneratorDependencyMissing(err.to_string())
                })?;
                let (_, chunk_data) = decode_chunk(&text).map_err(|err| {
                    TerrainFieldSourceError::GeneratorDependencyMissing(err.to_string())
                })?;
                tiles.insert(coord, chunk_data.heightfield);
            }
        }
        Ok(Self {
            layout,
            extent,
            tiles,
        })
    }

    pub fn from_heightfields(
        layout: ChunkLayout,
        extent: ChunkExtent,
        tiles: HashMap<ChunkCoord, Heightfield>,
    ) -> Self {
        Self {
            layout,
            extent,
            tiles,
        }
    }

    pub fn sample_height(&self, global_x: f32, global_z: f32) -> Option<f32> {
        let size = self.layout.chunk_size_units();
        let cx = (global_x / size).floor() as i32;
        let cz = (global_z / size).floor() as i32;
        if cx < self.extent.min.x
            || cz < self.extent.min.z
            || cx > self.extent.max.x
            || cz > self.extent.max.z
        {
            return None;
        }
        let hf = self.tiles.get(&ChunkCoord::new(cx, cz))?;
        let local_x = global_x - cx as f32 * size;
        let local_z = global_z - cz as f32 * size;
        Some(hf.sample(local_x, local_z))
    }

    pub fn sample_slope_degrees(&self, global_x: f32, global_z: f32) -> Option<f32> {
        let size = self.layout.chunk_size_units();
        let cx = (global_x / size).floor() as i32;
        let cz = (global_z / size).floor() as i32;
        let hf = self.tiles.get(&ChunkCoord::new(cx, cz))?;
        let local_x = global_x - cx as f32 * size;
        let local_z = global_z - cz as f32 * size;
        estimate_slope_degrees(hf, local_x, local_z)
    }
}

/// Optional biome dependency for stone and future generators.
#[derive(Debug, Clone)]
pub struct BiomeDependency {
    pub mask: BiomeMask,
}
