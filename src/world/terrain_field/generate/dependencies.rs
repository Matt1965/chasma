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
    min_height: f32,
    max_height: f32,
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
        Ok(Self::from_heightfields(layout, extent, tiles))
    }

    pub fn from_heightfields(
        layout: ChunkLayout,
        extent: ChunkExtent,
        tiles: HashMap<ChunkCoord, Heightfield>,
    ) -> Self {
        let (min_height, max_height) = compute_height_range(&tiles);
        Self {
            layout,
            extent,
            tiles,
            min_height,
            max_height,
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

    /// World-relative elevation in `[0, 1]` using packaged height span (not meters).
    pub fn normalized_elevation(&self, global_x: f32, global_z: f32) -> Option<f32> {
        let height = self.sample_height(global_x, global_z)?;
        let span = (self.max_height - self.min_height).max(1e-12);
        Some(((height - self.min_height) / span).clamp(0.0, 1.0))
    }

    /// How much lower this point is than its immediate neighborhood (valley/hollow bias).
    pub fn local_depression(&self, global_x: f32, global_z: f32) -> Option<f32> {
        let center = self.sample_height(global_x, global_z)?;
        let radius = 32.0;
        let ring = [
            self.sample_height(global_x + radius, global_z)?,
            self.sample_height(global_x - radius, global_z)?,
            self.sample_height(global_x, global_z + radius)?,
            self.sample_height(global_x, global_z - radius)?,
        ];
        let average = ring.iter().sum::<f32>() / ring.len() as f32;
        let span = (self.max_height - self.min_height).max(1e-12);
        Some(((average - center) / span * 6.0).clamp(0.0, 1.0))
    }
}

fn compute_height_range(tiles: &HashMap<ChunkCoord, Heightfield>) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for heightfield in tiles.values() {
        for &sample in heightfield.samples() {
            min = min.min(sample);
            max = max.max(sample);
        }
    }
    if min > max || !min.is_finite() || !max.is_finite() {
        (0.0, 1.0)
    } else {
        (min, max)
    }
}

/// Optional biome dependency for stone and future generators.
#[derive(Debug, Clone)]
pub struct BiomeDependency {
    pub mask: BiomeMask,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ramp_heightfield(max_height: f32) -> Heightfield {
        let samples = (0..9)
            .map(|index| (index / 3) as f32 / 2.0 * max_height)
            .collect();
        Heightfield::from_samples(3, 128.0, samples).unwrap()
    }

    #[test]
    fn normalized_elevation_uses_packaged_height_span() {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(0, 0),
        };
        let mut tiles = HashMap::new();
        tiles.insert(ChunkCoord::new(0, 0), ramp_heightfield(0.01));
        let dep = HeightfieldDependency::from_heightfields(layout, extent, tiles);
        let low = dep.normalized_elevation(128.0, 8.0).unwrap();
        let high = dep.normalized_elevation(128.0, 248.0).unwrap();
        assert!(low < 0.15);
        assert!(high > 0.85);
    }
}
