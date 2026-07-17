//! Partition a world-sized field raster into shared-edge chunk tiles (ADR-102).

use std::collections::BTreeMap;

use super::super::contract::{
    TERRAIN_FIELD_INTERVALS_PER_CHUNK, TERRAIN_FIELD_SAMPLE_SPACING_METERS,
    TERRAIN_FIELD_SAMPLES_PER_EDGE,
};
use super::super::id::TerrainFieldId;
use super::super::layer::TerrainFieldLayer;
use super::super::source::bounds::target_sample_dimensions;
use super::super::source_error::TerrainFieldSourceError;
use super::super::tile::TerrainFieldTile;
use crate::world::{ChunkCoord, ChunkExtent, ChunkId};

/// Row-major world raster: `row * width + col`, +X columns, +Z rows.
pub struct TerrainFieldWorldRaster {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u16>,
}

impl TerrainFieldWorldRaster {
    pub fn sample(&self, col: u32, row: u32) -> u16 {
        self.samples[row as usize * self.width as usize + col as usize]
    }

    pub fn from_vec(
        width: u32,
        height: u32,
        samples: Vec<u16>,
    ) -> Result<Self, TerrainFieldSourceError> {
        let expected = (width * height) as usize;
        if samples.len() != expected {
            return Err(TerrainFieldSourceError::TilePartitionFailed(format!(
                "expected {expected} samples, found {}",
                samples.len()
            )));
        }
        Ok(Self {
            width,
            height,
            samples,
        })
    }
}

pub fn partition_raster_to_tiles(
    raster: &TerrainFieldWorldRaster,
    extent: ChunkExtent,
    source_version: impl Into<String>,
) -> Result<BTreeMap<ChunkCoord, TerrainFieldTile>, TerrainFieldSourceError> {
    let (expected_w, expected_h) = target_sample_dimensions(extent);
    if raster.width != expected_w || raster.height != expected_h {
        return Err(TerrainFieldSourceError::TilePartitionFailed(format!(
            "raster {}x{} != expected {}x{}",
            raster.width, raster.height, expected_w, expected_h
        )));
    }

    let intervals = TERRAIN_FIELD_INTERVALS_PER_CHUNK as u32;
    let source_version = source_version.into();
    let mut tiles = BTreeMap::new();
    for z in extent.min.z..=extent.max.z {
        for x in extent.min.x..=extent.max.x {
            let chunk = ChunkCoord::new(x, z);
            let offset_x = (x - extent.min.x) as u32 * intervals;
            let offset_z = (z - extent.min.z) as u32 * intervals;
            let mut samples = Vec::with_capacity(
                (TERRAIN_FIELD_SAMPLES_PER_EDGE as usize)
                    * (TERRAIN_FIELD_SAMPLES_PER_EDGE as usize),
            );
            for local_row in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
                for local_col in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
                    samples.push(raster.sample(offset_x + local_col, offset_z + local_row));
                }
            }
            let tile = TerrainFieldTile {
                chunk,
                samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
                sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
                samples,
                tile_revision: 1,
                source_version: source_version.clone(),
            };
            let field_id = TerrainFieldId::new("_partition");
            tile.validate(&field_id)
                .map_err(|e| TerrainFieldSourceError::TilePartitionFailed(e.to_string()))?;
            if tiles.insert(chunk, tile).is_some() {
                return Err(TerrainFieldSourceError::TilePartitionFailed(format!(
                    "duplicate chunk ({x}, {z})"
                )));
            }
        }
    }
    Ok(tiles)
}

pub fn raster_to_layer(
    field_id: TerrainFieldId,
    raster: &TerrainFieldWorldRaster,
    extent: ChunkExtent,
    source_version: impl Into<String>,
) -> Result<TerrainFieldLayer, TerrainFieldSourceError> {
    let source_version = source_version.into();
    let tiles = partition_raster_to_tiles(raster, extent, source_version.clone())?;
    let mut layer = TerrainFieldLayer::new(field_id.clone(), source_version);
    for (coord, tile) in tiles {
        let _ = ChunkId::new(coord);
        layer
            .replace_tile(tile)
            .map_err(|e| TerrainFieldSourceError::TilePartitionFailed(e.to_string()))?;
    }
    layer
        .validate_shared_edges()
        .map_err(|e| TerrainFieldSourceError::SharedEdgeMismatch(e.to_string()))?;
    Ok(layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partitions_two_by_two_chunks_with_shared_edges() {
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        };
        let (w, h) = target_sample_dimensions(extent);
        let mut samples = vec![0u16; (w * h) as usize];
        for row in 0..h {
            for col in 0..w {
                samples[(row * w + col) as usize] = (col + row) as u16;
            }
        }
        let raster = TerrainFieldWorldRaster::from_vec(w, h, samples).unwrap();
        let tiles = partition_raster_to_tiles(&raster, extent, "test").unwrap();
        assert_eq!(tiles.len(), 4);
        let t00 = tiles.get(&ChunkCoord::new(0, 0)).unwrap();
        let t10 = tiles.get(&ChunkCoord::new(1, 0)).unwrap();
        assert_eq!(
            t00.sample_at_vertex(32, 0).unwrap(),
            t10.sample_at_vertex(0, 0).unwrap()
        );
    }
}
