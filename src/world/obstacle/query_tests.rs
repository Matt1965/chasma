use super::*;
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId, DoodadPlacementOverrides,
    DoodadSource, Heightfield, LocalPosition, create_doodad,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn catalogs() -> (DoodadCatalog, BuildingCatalog, FootprintCatalog) {
    (
        DoodadCatalog::default(),
        BuildingCatalog::default(),
        FootprintCatalog::default(),
    )
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
        None,
    )
    .unwrap();
}

#[test]
fn tree_definition_blocks_by_default() {
    let catalog = DoodadCatalog::default();
    let tree = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
    assert!(tree.blocks_movement);
    assert_eq!(tree.block_radius_meters, 1.0);
}

#[test]
fn blocks_when_footprint_overlaps() {
    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    insert_flat(&mut world, 0, 0);
    place_tree(&mut world, &doodad, pos(0, 0, 50.0, 50.0));
    assert!(is_position_blocked_by_doodads(
        &world,
        &doodad,
        &building,
        &footprint,
        pos(0, 0, 50.0, 50.0),
        0.5,
    ));
}

#[test]
fn exact_combined_radius_counts_as_blocked() {
    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    insert_flat(&mut world, 0, 0);
    place_tree(&mut world, &doodad, pos(0, 0, 50.0, 50.0));
    let tree = doodad.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
    let unit_radius = 0.5;
    let edge = tree.block_radius_meters + unit_radius;
    assert!(is_position_blocked_by_doodads(
        &world,
        &doodad,
        &building,
        &footprint,
        pos(0, 0, 50.0 + edge, 50.0),
        unit_radius,
    ));
}

#[test]
fn missing_definition_fails_closed_for_blocking_kind() {
    use crate::world::{DoodadId, DoodadKind, DoodadPlacement, DoodadRecord};

    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    insert_flat(&mut world, 0, 0);
    let position = pos(0, 0, 20.0, 20.0);
    let record = DoodadRecord::new(
        DoodadId::new(99),
        DoodadDefinitionId::new("missing_tree_def"),
        DoodadKind::Tree,
        DoodadPlacement::from_legacy(position, Quat::IDENTITY, Vec3::ONE).unwrap(),
        DoodadSource::Authored,
    );
    world
        .insert_doodad(ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();

    let result = query_obstacle_at_position(
        &world,
        &doodad,
        &building,
        &footprint,
        position,
        0.5,
    );
    assert!(result.blocked);
    assert!(matches!(
        result.error,
        Some(ObstacleQueryError::MissingDoodadDefinition { .. })
    ));
}
