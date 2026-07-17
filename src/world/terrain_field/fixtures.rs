//! Deterministic synthetic terrain field tiles for tests and dev bootstrap (ADR-101).

use bevy::prelude::*;

use super::contract::{TERRAIN_FIELD_SAMPLES_PER_EDGE, TERRAIN_FIELD_SAMPLES_PER_TILE};
use super::id::TerrainFieldId;
use super::store::TerrainFieldStore;
use super::tile::TerrainFieldTile;
use crate::world::ChunkCoord;

pub fn bootstrap_constant_field(
    store: &mut TerrainFieldStore,
    field_id: TerrainFieldId,
    chunk: ChunkCoord,
    value: u16,
) {
    let tile = TerrainFieldTile::new_constant(chunk, value, "synthetic_constant");
    store
        .replace_tile(field_id, tile, "synthetic_constant")
        .expect("synthetic tile");
}

pub fn bootstrap_x_gradient_field(
    store: &mut TerrainFieldStore,
    field_id: TerrainFieldId,
    chunk: ChunkCoord,
) {
    let mut samples = Vec::with_capacity(TERRAIN_FIELD_SAMPLES_PER_TILE);
    for row in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
        for col in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
            let t = col as f32 / (TERRAIN_FIELD_SAMPLES_PER_EDGE - 1) as f32;
            samples.push((t * 65_535.0).round() as u16);
            let _ = row;
        }
    }
    let tile = TerrainFieldTile {
        chunk,
        samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
        sample_spacing_meters: super::contract::TERRAIN_FIELD_SAMPLE_SPACING_METERS,
        samples,
        tile_revision: 1,
        source_version: "synthetic_x_gradient".to_string(),
    };
    store
        .replace_tile(field_id, tile, "synthetic_x_gradient")
        .expect("synthetic tile");
}

pub fn bootstrap_z_gradient_field(
    store: &mut TerrainFieldStore,
    field_id: TerrainFieldId,
    chunk: ChunkCoord,
) {
    let mut samples = Vec::with_capacity(TERRAIN_FIELD_SAMPLES_PER_TILE);
    for row in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
        for col in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
            let t = row as f32 / (TERRAIN_FIELD_SAMPLES_PER_EDGE - 1) as f32;
            samples.push((t * 65_535.0).round() as u16);
            let _ = col;
        }
    }
    let tile = TerrainFieldTile {
        chunk,
        samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
        sample_spacing_meters: super::contract::TERRAIN_FIELD_SAMPLE_SPACING_METERS,
        samples,
        tile_revision: 1,
        source_version: "synthetic_z_gradient".to_string(),
    };
    store
        .replace_tile(field_id, tile, "synthetic_z_gradient")
        .expect("synthetic tile");
}

pub fn bootstrap_diagonal_gradient_field(
    store: &mut TerrainFieldStore,
    field_id: TerrainFieldId,
    chunk: ChunkCoord,
) {
    let mut samples = Vec::with_capacity(TERRAIN_FIELD_SAMPLES_PER_TILE);
    for row in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
        for col in 0..TERRAIN_FIELD_SAMPLES_PER_EDGE as u32 {
            let t = (col + row) as f32 / (2 * (TERRAIN_FIELD_SAMPLES_PER_EDGE - 1) as u32) as f32;
            samples.push((t * 65_535.0).round() as u16);
        }
    }
    let tile = TerrainFieldTile {
        chunk,
        samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
        sample_spacing_meters: super::contract::TERRAIN_FIELD_SAMPLE_SPACING_METERS,
        samples,
        tile_revision: 1,
        source_version: "synthetic_diagonal".to_string(),
    };
    store
        .replace_tile(field_id, tile, "synthetic_diagonal")
        .expect("synthetic tile");
}

/// Populate all four initial fields across an authored extent with distinct synthetic patterns.
pub fn bootstrap_dev_synthetic_fields(
    store: &mut TerrainFieldStore,
    extent_min: ChunkCoord,
    extent_max: ChunkCoord,
) {
    for z in extent_min.z..=extent_max.z {
        for x in extent_min.x..=extent_max.x {
            let chunk = ChunkCoord::new(x, z);
            bootstrap_constant_field(store, TerrainFieldId::new("water"), chunk, 20_000);
            bootstrap_x_gradient_field(store, TerrainFieldId::new("iron"), chunk);
            bootstrap_z_gradient_field(store, TerrainFieldId::new("copper"), chunk);
            bootstrap_diagonal_gradient_field(store, TerrainFieldId::new("stone"), chunk);
        }
    }
}
