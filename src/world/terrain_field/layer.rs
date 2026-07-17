//! One terrain field layer across the world (ADR-101).

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::contract::{
    TERRAIN_FIELD_BYTES_PER_TILE, TERRAIN_FIELD_SAMPLE_SPACING_METERS,
    TERRAIN_FIELD_SAMPLES_PER_EDGE,
};
use super::error::{SharedEdgeAxis, TerrainFieldStorageError};
use super::id::TerrainFieldId;
use super::tile::TerrainFieldTile;
use crate::world::{ChunkCoord, ChunkId};

/// All chunk tiles for one [`TerrainFieldId`].
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TerrainFieldLayer {
    pub field_id: TerrainFieldId,
    pub samples_per_edge: u16,
    pub sample_spacing_meters: f32,
    pub tiles: BTreeMap<ChunkCoord, TerrainFieldTile>,
    pub layer_revision: u64,
    pub source_version: String,
}

impl TerrainFieldLayer {
    pub fn new(field_id: TerrainFieldId, source_version: impl Into<String>) -> Self {
        Self {
            field_id,
            samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
            sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
            tiles: BTreeMap::new(),
            layer_revision: 0,
            source_version: source_version.into(),
        }
    }

    pub fn insert_tile(&mut self, tile: TerrainFieldTile) -> Result<(), TerrainFieldStorageError> {
        tile.validate(&self.field_id)?;
        if tile.chunk != tile.chunk_id().coord() {
            return Err(TerrainFieldStorageError::TileChunkMismatch {
                tile_chunk: tile.chunk,
                key_chunk: tile.chunk,
            });
        }
        let chunk_coord = tile.chunk;
        if self.tiles.insert(chunk_coord, tile).is_some() {
            return Err(TerrainFieldStorageError::DuplicateTile {
                field_id: self.field_id.clone(),
                chunk: ChunkId::new(chunk_coord),
            });
        }
        self.layer_revision = self.layer_revision.saturating_add(1);
        Ok(())
    }

    pub fn replace_tile(
        &mut self,
        mut tile: TerrainFieldTile,
    ) -> Result<(), TerrainFieldStorageError> {
        tile.validate(&self.field_id)?;
        if let Some(existing) = self.tiles.get(&tile.chunk) {
            tile.tile_revision = existing.tile_revision.saturating_add(1);
        }
        self.tiles.insert(tile.chunk, tile);
        self.layer_revision = self.layer_revision.saturating_add(1);
        Ok(())
    }

    pub fn remove_tile(&mut self, chunk: ChunkCoord) -> Option<TerrainFieldTile> {
        let removed = self.tiles.remove(&chunk);
        if removed.is_some() {
            self.layer_revision = self.layer_revision.saturating_add(1);
        }
        removed
    }

    pub fn get_tile(&self, chunk: ChunkCoord) -> Option<&TerrainFieldTile> {
        self.tiles.get(&chunk)
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn sorted_chunk_coords(&self) -> Vec<ChunkCoord> {
        self.tiles.keys().copied().collect()
    }

    pub fn memory_bytes(&self) -> usize {
        self.tiles.len() * TERRAIN_FIELD_BYTES_PER_TILE
    }

    pub fn validate_shared_edges(&self) -> Result<(), TerrainFieldStorageError> {
        for (coord, tile) in &self.tiles {
            let east = ChunkCoord::new(coord.x + 1, coord.z);
            if let Some(neighbor) = self.tiles.get(&east) {
                validate_edge_pair(
                    &self.field_id,
                    SharedEdgeAxis::EastWest,
                    *coord,
                    east,
                    tile,
                    neighbor,
                    true,
                )?;
            }
            let north = ChunkCoord::new(coord.x, coord.z + 1);
            if let Some(neighbor) = self.tiles.get(&north) {
                validate_edge_pair(
                    &self.field_id,
                    SharedEdgeAxis::NorthSouth,
                    *coord,
                    north,
                    tile,
                    neighbor,
                    true,
                )?;
            }
        }
        Ok(())
    }
}

fn validate_edge_pair(
    field_id: &TerrainFieldId,
    axis: SharedEdgeAxis,
    chunk_a: ChunkCoord,
    chunk_b: ChunkCoord,
    tile_a: &TerrainFieldTile,
    tile_b: &TerrainFieldTile,
    a_is_lower: bool,
) -> Result<(), TerrainFieldStorageError> {
    let spe = tile_a.samples_per_edge as u32;
    match axis {
        SharedEdgeAxis::EastWest => {
            let col_a = spe - 1;
            let col_b = 0;
            for row in 0..spe {
                let va = tile_a.sample_at_vertex(col_a, row).unwrap();
                let vb = tile_b.sample_at_vertex(col_b, row).unwrap();
                if va != vb {
                    return Err(TerrainFieldStorageError::SharedEdgeMismatch {
                        field_id: field_id.clone(),
                        axis,
                        chunk_a,
                        chunk_b,
                        index: row as u16,
                        value_a: va,
                        value_b: vb,
                    });
                }
            }
        }
        SharedEdgeAxis::NorthSouth => {
            let row_a = spe - 1;
            let row_b = 0;
            for col in 0..spe {
                let va = tile_a.sample_at_vertex(col, row_a).unwrap();
                let vb = tile_b.sample_at_vertex(col, row_b).unwrap();
                if va != vb {
                    return Err(TerrainFieldStorageError::SharedEdgeMismatch {
                        field_id: field_id.clone(),
                        axis,
                        chunk_a,
                        chunk_b,
                        index: col as u16,
                        value_a: va,
                        value_b: vb,
                    });
                }
            }
        }
    }
    let _ = a_is_lower;
    Ok(())
}
