use std::collections::HashSet;

use bevy::prelude::Vec3;

use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, Road,
    RoadControlPoint, RoadId, RoadNetwork, RoadStyleId, RoadStyleOverrides, WorldData, WorldPosition,
    road::deformation::{
        RoadDeformationStore, affected_chunk_ids_for_network, rebake_road_deformation_for_chunks,
        sync_store_tiles_to_chunks,
    },
    terrain::{try_sample_base_height_at_position, try_sample_height_at_position},
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn flat_chunk(height: f32) -> ChunkData {
    let heightfield = Heightfield::from_samples(5, 64.0, vec![height; 25]).unwrap();
    ChunkData::new(heightfield, Vec::new())
}

fn horizontal_road(id: &str, x0: f32, z: f32, x1: f32, style: RoadStyleId) -> Road {
    Road {
        id: RoadId::new(id),
        display_name: String::new(),
        style,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![
            RoadControlPoint::new(x0, z),
            RoadControlPoint::new(x1, z),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    }
}

fn network_with_roads(roads: Vec<Road>) -> RoadNetwork {
    let mut network = RoadNetwork::empty();
    for road in roads {
        network.roads.insert(road.id.clone(), road);
    }
    network
}

fn world_with_chunk(coord: ChunkCoord, chunk: ChunkData) -> WorldData {
    let mut world = WorldData::new(layout());
    world.insert(ChunkId::new(coord), chunk);
    world
}

fn position(coord: ChunkCoord, x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(coord, LocalPosition::new(Vec3::new(x, 0.0, z)))
}

fn rebake_single(
    world: &WorldData,
    store: &mut RoadDeformationStore,
    network: &RoadNetwork,
    chunk_id: ChunkId,
) {
    let mut chunks = HashSet::new();
    chunks.insert(chunk_id);
    rebake_road_deformation_for_chunks(world, store, network, layout(), &chunks);
}

#[test]
fn flat_terrain_road_applies_depression_only_near_road() {
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(10.0));
    let network = network_with_roads(vec![horizontal_road("r1", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    sync_store_tiles_to_chunks(&store, &mut world, &HashSet::from([chunk_id]));

    let on_road = position(ChunkCoord::new(0, 0), 128.0, 128.0);
    let off_road = position(ChunkCoord::new(0, 0), 8.0, 8.0);
    let base = try_sample_base_height_at_position(&world, on_road).unwrap();
    let effective = try_sample_height_at_position(&world, on_road).unwrap();
    assert!(effective < base);
    assert!(
        (try_sample_height_at_position(&world, off_road).unwrap()
            - try_sample_base_height_at_position(&world, off_road).unwrap())
            .abs()
            < 1e-5
    );
}

#[test]
fn bumpy_centerline_is_smoothed_along_road() {
    let mut heights = Vec::new();
    for row in 0..5 {
        for col in 0..5 {
            let bump = if col == 2 { (row % 2) as f32 * 0.5 } else { 0.0 };
            heights.push(10.0 + bump);
        }
    }
    let chunk = ChunkData::new(Heightfield::from_samples(5, 64.0, heights).unwrap(), Vec::new());
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), chunk);
    let network = network_with_roads(vec![horizontal_road("r1", 0.0, 128.0, 256.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    sync_store_tiles_to_chunks(&store, &mut world, &HashSet::from([chunk_id]));

    let center = position(ChunkCoord::new(0, 0), 128.0, 128.0);
    let base = try_sample_base_height_at_position(&world, center).unwrap();
    let effective = try_sample_height_at_position(&world, center).unwrap();
    assert!((effective - base).abs() < 0.45);
}

#[test]
fn large_hill_is_preserved_along_road() {
    let mut heights = Vec::new();
    for row in 0..5 {
        for _col in 0..5 {
            heights.push(row as f32 * 4.0);
        }
    }
    let chunk = ChunkData::new(Heightfield::from_samples(5, 64.0, heights).unwrap(), Vec::new());
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), chunk);
    let network =
        network_with_roads(vec![horizontal_road("r1", 64.0, 0.0, 192.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    sync_store_tiles_to_chunks(&store, &mut world, &HashSet::from([chunk_id]));

    let low = position(ChunkCoord::new(0, 0), 64.0, 0.0);
    let high = position(ChunkCoord::new(0, 0), 192.0, 256.0);
    let low_eff = try_sample_height_at_position(&world, low).unwrap();
    let high_eff = try_sample_height_at_position(&world, high).unwrap();
    assert!(high_eff > low_eff + 8.0);
}

#[test]
fn shoulder_falloff_reaches_zero_outside_influence() {
    let world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(5.0));
    let network = network_with_roads(vec![horizontal_road("r1", 100.0, 128.0, 156.0, RoadStyleId::Trail)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    let tile = store.tiles.get(&chunk_id).expect("tile");
    let corner = tile.delta_at_vertex(0, 0);
    assert!(corner.abs() < 1e-5);
}

#[test]
fn style_override_affects_deformation() {
    let world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(0.0));
    let mut road = horizontal_road("r1", 96.0, 128.0, 160.0, RoadStyleId::Trail);
    road.style_overrides.depression_m = Some(0.2);
    let network = network_with_roads(vec![road]);
    let mut store = RoadDeformationStore::default();
    rebake_single(
        &world,
        &mut store,
        &network,
        ChunkId::new(ChunkCoord::new(0, 0)),
    );
    let on_road = position(ChunkCoord::new(0, 0), 128.0, 128.0);
    let mut world_mut = world;
    sync_store_tiles_to_chunks(
        &store,
        &mut world_mut,
        &HashSet::from([ChunkId::new(ChunkCoord::new(0, 0))]),
    );
    let effective = try_sample_height_at_position(&world_mut, on_road).unwrap();
    assert!(effective < -0.08);
}

#[test]
fn overlapping_road_rebake_is_deterministic() {
    let world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(0.0));
    let network = network_with_roads(vec![
        horizontal_road("east_west", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad),
        Road {
            id: RoadId::new("north_south"),
            display_name: String::new(),
            style: RoadStyleId::DirtRoad,
            style_overrides: RoadStyleOverrides::default(),
            control_points: vec![
                RoadControlPoint::new(128.0, 32.0),
                RoadControlPoint::new(128.0, 224.0),
            ],
            start_attachment: None,
            end_attachment: None,
            tee_attachments: Vec::new(),
        },
    ]);
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    let mut first = RoadDeformationStore::default();
    let mut second = RoadDeformationStore::default();
    rebake_single(&world, &mut first, &network, chunk_id);
    rebake_single(&world, &mut second, &network, chunk_id);
    assert_eq!(first.tiles, second.tiles);
}

#[test]
fn chunk_border_heights_match_for_road_across_chunks() {
    let mut world = WorldData::new(layout());
    let west = flat_chunk(2.0);
    let east = flat_chunk(2.0);
    world.insert(ChunkId::new(ChunkCoord::new(0, 0)), west);
    world.insert(ChunkId::new(ChunkCoord::new(1, 0)), east);
    let network = network_with_roads(vec![horizontal_road("r1", 200.0, 128.0, 300.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let dirty = affected_chunk_ids_for_network(&network, layout());
    rebake_road_deformation_for_chunks(&world, &mut store, &network, layout(), &dirty);
    sync_store_tiles_to_chunks(&store, &mut world, &dirty);

    let west_edge = position(ChunkCoord::new(0, 0), 256.0, 128.0);
    let east_edge = position(ChunkCoord::new(1, 0), 0.0, 128.0);
    let west_h = try_sample_height_at_position(&world, west_edge).unwrap();
    let east_h = try_sample_height_at_position(&world, east_edge).unwrap();
    assert!((west_h - east_h).abs() < 0.05);
}

#[test]
fn moving_road_restores_old_chunk_to_base() {
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(3.0));
    let road_v1 = horizontal_road("r1", 80.0, 128.0, 176.0, RoadStyleId::DirtRoad);
    let network_v1 = network_with_roads(vec![road_v1]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network_v1, chunk_id);
    sync_store_tiles_to_chunks(&store, &mut world, &HashSet::from([chunk_id]));
    let center = position(ChunkCoord::new(0, 0), 128.0, 128.0);
    assert!(
        (try_sample_height_at_position(&world, center).unwrap()
            - try_sample_base_height_at_position(&world, center).unwrap())
            .abs()
            > 1e-4
    );

    let road_v2 = horizontal_road("r1", 8.0, 8.0, 40.0, RoadStyleId::DirtRoad);
    let network_v2 = network_with_roads(vec![road_v2]);
    rebake_single(&world, &mut store, &network_v2, chunk_id);
    sync_store_tiles_to_chunks(&store, &mut world, &HashSet::from([chunk_id]));
    assert!(
        (try_sample_height_at_position(&world, center).unwrap()
            - try_sample_base_height_at_position(&world, center).unwrap())
            .abs()
            < 1e-4
    );
}
