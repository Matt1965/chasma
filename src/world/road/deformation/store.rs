use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::{ChunkCoord, ChunkId, ChunkLayout, WorldData};

use super::tile::RoadHeightDeltaTile;

pub const ROAD_DEFORMATION_BAKE_VERSION: u32 = 1;

/// Sparse derived road deformation authority keyed by terrain chunk.
#[derive(Debug, Clone, Default, Resource, Reflect)]
pub struct RoadDeformationStore {
    pub bake_version: u32,
    pub network_fingerprint: u64,
    pub revision: u64,
    pub tiles: HashMap<ChunkId, RoadHeightDeltaTile>,
}

impl RoadDeformationStore {
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.revision = 0;
    }

    pub fn delta_at_position(
        &self,
        world: &WorldData,
        position: crate::world::WorldPosition,
    ) -> f32 {
        let chunk_id = ChunkId::new(position.chunk);
        let Some(tile) = self.tiles.get(&chunk_id) else {
            return 0.0;
        };
        let Some(chunk) = world.get(chunk_id) else {
            return 0.0;
        };
        if tile.samples_per_edge != chunk.heightfield.samples_per_edge()
            || (tile.spacing_meters - chunk.heightfield.spacing_meters()).abs() > 1e-5
        {
            return 0.0;
        }
        tile.sample_delta(position.local.0.x, position.local.0.z)
            .unwrap_or(0.0)
    }

    pub fn apply_tile_to_chunk(&self, chunk_id: ChunkId, chunk: &mut crate::world::ChunkData) {
        chunk.road_height_delta = self
            .tiles
            .get(&chunk_id)
            .cloned()
            .filter(|tile| !tile.is_effectively_zero());
    }

    pub fn set_tile(&mut self, chunk_id: ChunkId, tile: RoadHeightDeltaTile) {
        if tile.is_effectively_zero() {
            self.tiles.remove(&chunk_id);
        } else {
            self.tiles.insert(chunk_id, tile);
        }
        self.revision += 1;
    }

    pub fn remove_chunks(&mut self, chunk_ids: &HashSet<ChunkId>) {
        for chunk_id in chunk_ids {
            self.tiles.remove(chunk_id);
        }
        self.revision += 1;
    }
}

/// RON document for persisted baked deformation tiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadDeformationBakeDocument {
    pub bake_version: u32,
    pub network_fingerprint: u64,
    pub tiles: HashMap<ChunkCoord, RoadHeightDeltaTile>,
}

pub fn chunk_ids_from_coords(coords: &HashSet<ChunkCoord>) -> HashSet<ChunkId> {
    coords.iter().map(|coord| ChunkId::new(*coord)).collect()
}

pub fn chunk_coords_from_ids(ids: &HashSet<ChunkId>) -> HashSet<ChunkCoord> {
    ids.iter().map(|id| id.coord()).collect()
}

pub fn sync_store_tiles_to_resident_chunks(store: &RoadDeformationStore, world: &mut WorldData) {
    for (chunk_id, chunk) in world.chunks_mut() {
        store.apply_tile_to_chunk(*chunk_id, chunk);
    }
}

pub fn sync_store_tiles_to_chunks(
    store: &RoadDeformationStore,
    world: &mut WorldData,
    chunk_ids: &HashSet<ChunkId>,
) {
    for chunk_id in chunk_ids {
        if let Some(chunk) = world.get_mut(*chunk_id) {
            store.apply_tile_to_chunk(*chunk_id, chunk);
        }
    }
}

pub fn affected_chunk_coords_for_bounds(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    layout: ChunkLayout,
) -> HashSet<ChunkCoord> {
    let chunk_size = layout.chunk_size_meters;
    let min_chunk_x = (min_x / chunk_size).floor() as i32;
    let max_chunk_x = (max_x / chunk_size).floor() as i32;
    let min_chunk_z = (min_z / chunk_size).floor() as i32;
    let max_chunk_z = (max_z / chunk_size).floor() as i32;
    let mut coords = HashSet::new();
    for x in min_chunk_x..=max_chunk_x {
        for z in min_chunk_z..=max_chunk_z {
            coords.insert(ChunkCoord::new(x, z));
        }
    }
    coords
}
