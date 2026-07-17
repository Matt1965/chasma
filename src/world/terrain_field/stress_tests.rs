//! TF6 stress and determinism checks for terrain field queries.

use crate::world::{ChunkCoord, ChunkExtent, LocalPosition, WorldPosition};
use crate::world::{
    TerrainFieldCatalog, TerrainFieldId, WorldConfig, WorldData, bootstrap_constant_field,
    sample_terrain_field_at,
};
use bevy::prelude::Vec3;

fn stress_world() -> (WorldData, TerrainFieldCatalog) {
    let layout = WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(31, 31),
    });
    for z in 0..=31 {
        for x in 0..=31 {
            bootstrap_constant_field(
                world.terrain_fields_mut(),
                TerrainFieldId::new("water"),
                ChunkCoord::new(x, z),
                20_000 + ((x + z) as u16) * 100,
            );
        }
    }
    (world, TerrainFieldCatalog::default())
}

#[test]
#[ignore = "stress benchmark — run with cargo test --ignored"]
fn one_million_point_queries_complete() {
    let (world, catalog) = stress_world();
    let field = TerrainFieldId::new("water");
    let mut last = None;
    for i in 0..1_000_000u32 {
        let x = (i % 256) as f32;
        let z = ((i / 256) % 256) as f32;
        let position = WorldPosition::new(
            ChunkCoord::new((x / 256.0) as i32, (z / 256.0) as i32),
            LocalPosition::new(Vec3::new(x % 256.0, 0.0, z % 256.0)),
        );
        let sample = sample_terrain_field_at(&world, &catalog, &field, position);
        if sample.availability.is_available() {
            last = Some(sample.value);
        }
    }
    assert!(last.is_some());
}

#[test]
fn query_repeat_is_deterministic() {
    let (world, catalog) = stress_world();
    let field = TerrainFieldId::new("water");
    let position = WorldPosition::new(
        ChunkCoord::new(1, 1),
        LocalPosition::new(Vec3::new(40.0, 0.0, 40.0)),
    );
    let a = sample_terrain_field_at(&world, &catalog, &field, position);
    let b = sample_terrain_field_at(&world, &catalog, &field, position);
    assert_eq!(a, b);
}
