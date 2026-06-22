//! Circle overlap tests for doodad movement obstacles (ADR-031).

use bevy::prelude::*;

use crate::world::{
    ChunkCoord, ChunkId, DoodadCatalog, DoodadId, WorldData, WorldPosition,
};

/// Return `true` when a unit footprint at `position` overlaps a blocking doodad.
///
/// Checks the owning chunk and eight neighbors. Uses XZ distance only:
/// `unit_radius + doodad.block_radius_meters`.
pub fn is_position_blocked_by_doodads(
    world: &WorldData,
    catalog: &DoodadCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> bool {
    blocking_doodad_at_position(world, catalog, position, radius_meters).is_some()
}

/// The first blocking doodad id overlapping `position`, if any (deterministic order).
pub fn blocking_doodad_at_position(
    world: &WorldData,
    catalog: &DoodadCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> Option<DoodadId> {
    let layout = world.layout();
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);

    let mut chunks: Vec<ChunkCoord> = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            chunks.push(ChunkCoord::new(position.chunk.x + dx, position.chunk.z + dz));
        }
    }
    chunks.sort_by_key(|coord| (coord.x, coord.z));

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        let Some(store) = world.doodads_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            let Some(definition) = catalog.get(&record.definition_id) else {
                continue;
            };
            if !definition.blocks_movement {
                continue;
            }
            let doodad_global = record.placement.position.to_global(layout);
            let doodad_xz = Vec2::new(doodad_global.x, doodad_global.z);
            let combined = radius_meters + definition.block_radius_meters;
            if center_xz.distance(doodad_xz) < combined {
                return Some(record.id);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_doodad, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
        DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(chunk_x: i32, chunk_z: i32, x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(chunk_x, chunk_z),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn insert_flat(world: &mut WorldData, x: i32, z: i32) {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn place_tree(world: &mut WorldData, catalog: &DoodadCatalog, position: WorldPosition) {
        create_doodad(
            catalog,
            world,
            &DoodadDefinitionId::new("tree_oak"),
            position,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
    }

    #[test]
    fn tree_definition_blocks_by_default() {
        let catalog = DoodadCatalog::default();
        let tree = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
        assert!(tree.blocks_movement);
        assert_eq!(tree.block_radius_meters, tree.placement_radius_meters);

        let bush = catalog.get(&DoodadDefinitionId::new("bush_scrub")).unwrap();
        assert!(!bush.blocks_movement);
    }

    #[test]
    fn blocks_when_footprint_overlaps() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(0, 0, 50.0, 50.0));

        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 50.0, 50.0),
            0.5,
        ));
    }

    #[test]
    fn allows_when_outside_combined_radius() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(0, 0, 50.0, 50.0));
        let tree = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();

        let unit_pos = pos(0, 0, 50.0 + tree.block_radius_meters + 1.0, 50.0);
        assert!(!is_position_blocked_by_doodads(
            &world,
            &catalog,
            unit_pos,
            0.5,
        ));
    }

    #[test]
    fn neighbor_chunk_obstacle_blocks_near_border() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(1, 0, 2.0, 128.0));

        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 255.0, 128.0),
            0.5,
        ));
    }

    #[test]
    fn non_blocking_doodad_does_not_block() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("bush_scrub"),
            pos(0, 0, 10.0, 10.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        assert!(!is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 10.0, 10.0),
            0.5,
        ));
    }

    #[test]
    fn uses_world_data_not_render_entities() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(0, 0, 20.0, 20.0));
        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 20.0, 20.0),
            0.5,
        ));
    }
}
