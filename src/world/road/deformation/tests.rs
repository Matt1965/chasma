use std::collections::HashSet;

use bevy::prelude::Vec3;

use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, Road,
    RoadControlPoint, RoadId, RoadNetwork, RoadStyleId, RoadStyleOverrides, WorldData, WorldPosition,
    road::deformation::{
        RoadDeformationStore, affected_chunk_ids_for_network, depression_meters_to_heightfield_units,
        rebake_road_deformation_for_chunks,
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

fn vertical_road(id: &str, x: f32, z0: f32, z1: f32, style: RoadStyleId) -> Road {
    Road {
        id: RoadId::new(id),
        display_name: String::new(),
        style,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![
            RoadControlPoint::new(x, z0),
            RoadControlPoint::new(x, z1),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    }
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

fn rebake(
    world: &WorldData,
    store: &mut RoadDeformationStore,
    network: &RoadNetwork,
    chunk_ids: &HashSet<ChunkId>,
) {
    rebake_road_deformation_for_chunks(world, store, network, layout(), chunk_ids, None);
}

fn rebake_single(
    world: &WorldData,
    store: &mut RoadDeformationStore,
    network: &RoadNetwork,
    chunk_id: ChunkId,
) {
    let mut chunks = HashSet::new();
    chunks.insert(chunk_id);
    rebake(world, store, network, &chunks);
}

fn tile_nonzero_count(tile: &crate::world::RoadHeightDeltaTile) -> usize {
    tile.deltas.iter().filter(|d| d.abs() > 1e-5).count()
}

fn tile_delta_range(tile: &crate::world::RoadHeightDeltaTile) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for delta in &tile.deltas {
        if delta.is_finite() {
            min = min.min(*delta);
            max = max.max(*delta);
        }
    }
    (min, max)
}

#[test]
fn flat_120m_road_centerline_delta_matches_depression() {
    let base_height = 120.0;
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(base_height));
    let network =
        network_with_roads(vec![horizontal_road("r1", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad)]);
    let depression_m = network
        .style_defaults(RoadStyleId::DirtRoad)
        .expect("style")
        .depression_m;
    let depression_hf = depression_meters_to_heightfield_units(depression_m, 60.0);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    sync_store(&store, &mut world, chunk_id);

    let center = position(ChunkCoord::new(0, 0), 128.0, 128.0);
    let base = try_sample_base_height_at_position(&world, center).unwrap();
    let effective = try_sample_height_at_position(&world, center).unwrap();
    let tile = store.tiles.get(&chunk_id).expect("tile");
    let stored_delta = tile.sample_delta(128.0, 128.0).unwrap();

    assert!((base - base_height).abs() < 1e-3);
    assert!((stored_delta - (effective - base)).abs() < 1e-4);
    assert!(stored_delta < 0.0);
    assert!(stored_delta > -depression_hf * 2.0);
    assert!((effective - (base - depression_hf)).abs() < depression_hf * 2.0);
}

#[test]
fn elevation_translation_preserves_road_deltas() {
    fn run_at_base(base_height: f32) -> (f32, f32) {
        let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(base_height));
        let network =
            network_with_roads(vec![horizontal_road("r1", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad)]);
        let mut store = RoadDeformationStore::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        rebake_single(&world, &mut store, &network, chunk_id);
        let tile = store.tiles.get(&chunk_id).expect("tile");
        let delta = tile.sample_delta(128.0, 128.0).unwrap();
        sync_store(&store, &mut world, chunk_id);
        let center = position(ChunkCoord::new(0, 0), 128.0, 128.0);
        let effective = try_sample_height_at_position(&world, center).unwrap();
        (delta, effective)
    }

    let (delta_a, effective_a) = run_at_base(10.0);
    let (delta_b, effective_b) = run_at_base(110.0);
    assert!((delta_a - delta_b).abs() < 1e-3, "a={} b={}", delta_a, delta_b);
    assert!((effective_b - effective_a - 100.0).abs() < 1e-2);
}

#[test]
fn cross_slope_road_cuts_uphill_and_fills_downhill() {
    let spe = 129;
    let spacing = 2.0;
    let mut heights = Vec::new();
    for row in 0..spe {
        for _col in 0..spe {
            heights.push(120.0 + (row as f32 - 64.0) * 10.0);
        }
    }
    let chunk =
        ChunkData::new(Heightfield::from_samples(spe, spacing, heights).unwrap(), Vec::new());
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), chunk);
    let network =
        network_with_roads(vec![horizontal_road("r1", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    sync_store(&store, &mut world, chunk_id);

    // Cross-slope varies with Z; road runs east-west at z=128. Sample perpendicular to the road.
    let center = position(ChunkCoord::new(0, 0), 128.0, 128.0);
    let uphill = position(ChunkCoord::new(0, 0), 128.0, 130.0);
    let downhill = position(ChunkCoord::new(0, 0), 128.0, 126.0);

    let uphill_delta = try_sample_height_at_position(&world, uphill).unwrap()
        - try_sample_base_height_at_position(&world, uphill).unwrap();
    let center_delta = try_sample_height_at_position(&world, center).unwrap()
        - try_sample_base_height_at_position(&world, center).unwrap();
    let downhill_delta = try_sample_height_at_position(&world, downhill).unwrap()
        - try_sample_base_height_at_position(&world, downhill).unwrap();

    assert!(uphill_delta < 0.0, "uphill {}", uphill_delta);
    assert!(center_delta < 0.0, "center {}", center_delta);
    assert!(downhill_delta > 0.0, "downhill {}", downhill_delta);
    assert!(uphill_delta < center_delta, "uphill {} center {}", uphill_delta, center_delta);
    assert!(downhill_delta > center_delta, "downhill {} center {}", downhill_delta, center_delta);
    assert!(uphill_delta.abs() > center_delta.abs());
    assert!(downhill_delta.abs() > center_delta.abs());
}

#[test]
fn shoulder_delta_fades_between_center_and_outside() {
    let base_height = 120.0;
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(base_height));
    let network =
        network_with_roads(vec![horizontal_road("r1", 100.0, 128.0, 156.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    let tile = store.tiles.get(&chunk_id).expect("tile");

    let center = tile.sample_delta(128.0, 128.0).unwrap();
    let shoulder = tile.sample_delta(140.0, 128.0).unwrap();
    let outside = tile.sample_delta(8.0, 8.0).unwrap();

    assert!(center.abs() > shoulder.abs());
    assert!(outside.abs() < 1e-5);
}

#[test]
fn stored_tile_values_are_deltas_not_absolute_heights() {
    let base_height = 120.0;
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(base_height));
    let network =
        network_with_roads(vec![horizontal_road("r1", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    sync_store(&store, &mut world, chunk_id);
    let chunk = world.get(chunk_id).expect("chunk");
    let tile = chunk.road_height_delta.as_ref().expect("tile");

    for row in 0..tile.samples_per_edge {
        for col in 0..tile.samples_per_edge {
            let base = chunk.heightfield.height_at_vertex(col, row);
            let delta = tile.delta_at_vertex(col, row);
            if delta.abs() > 1e-5 {
                assert!(delta.abs() < 5.0, "catastrophic delta {} at {},{}", delta, col, row);
                assert!((base + delta - chunk.effective_height_at_vertex(col, row)).abs() < 1e-5);
            }
        }
    }
}

#[test]
fn missing_centerline_heights_never_default_to_zero() {
    let filled = crate::world::road::deformation::bake::fill_missing_centerline_base_heights_for_test(&[
        None,
        None,
        Some(120.0),
        None,
        Some(130.0),
    ])
    .expect("filled");
    assert!((filled[0] - 120.0).abs() < 1e-4);
    assert!((filled[1] - 120.0).abs() < 1e-4);
    assert!((filled[4] - 130.0).abs() < 1e-4);
}

#[test]
fn single_known_centerline_sample_is_rejected() {
    let filled = crate::world::road::deformation::bake::fill_missing_centerline_base_heights_for_test(
        &[Some(0.0), None, None, None, None],
    );
    assert!(filled.is_none());
}

#[test]
fn flat_terrain_road_applies_depression_only_near_road() {
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(10.0));
    let network = network_with_roads(vec![horizontal_road("r1", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    sync_store(&store, &mut world, chunk_id);

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
fn high_elevation_endpoints_stay_near_base_not_world_zero() {
    let base_height = 120.0;
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), flat_chunk(base_height));
    let network = network_with_roads(vec![horizontal_road("r1", 0.0, 128.0, 256.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    let tile = store.tiles.get(&chunk_id).expect("tile");
    let (min_delta, max_delta) = tile_delta_range(tile);
    assert!(min_delta < -0.01);
    assert!(min_delta > -1.0);
    assert!(max_delta < 0.1);
    sync_store(&store, &mut world, chunk_id);
    let start = position(ChunkCoord::new(0, 0), 0.0, 128.0);
    let end = position(ChunkCoord::new(0, 0), 256.0, 128.0);
    let start_eff = try_sample_height_at_position(&world, start).unwrap();
    let end_eff = try_sample_height_at_position(&world, end).unwrap();
    assert!(start_eff > base_height - 1.0);
    assert!(end_eff > base_height - 1.0);
    assert!(start_eff < base_height);
    assert!(end_eff < base_height);
}

#[test]
fn multi_chunk_road_deforms_all_intersected_chunks() {
    let mut world = WorldData::new(layout());
    for x in 0..3 {
        world.insert(ChunkId::new(ChunkCoord::new(x, 0)), flat_chunk(50.0));
    }
    let network = network_with_roads(vec![horizontal_road("long", 32.0, 128.0, 700.0, RoadStyleId::DirtRoad)]);
    let dirty = affected_chunk_ids_for_network(&network, layout());
    assert!(dirty.len() >= 3);
    let mut store = RoadDeformationStore::default();
    rebake(&world, &mut store, &network, &dirty);
    for x in 0..3 {
        let chunk_id = ChunkId::new(ChunkCoord::new(x, 0));
        let tile = store.tiles.get(&chunk_id).expect("tile for resident chunk");
        assert!(tile_nonzero_count(tile) > 0);
        let (min_delta, max_delta) = tile_delta_range(tile);
        assert!(min_delta > -0.5);
        assert!(max_delta < 0.5);
        assert!(tile.delta_at_vertex(2, 2).abs() > 1e-5 || tile.delta_at_vertex(1, 2).abs() > 1e-5);
    }
}

#[test]
fn partial_residency_does_not_create_endpoint_holes() {
    let mut world = WorldData::new(layout());
    world.insert(ChunkId::new(ChunkCoord::new(1, 0)), flat_chunk(80.0));
    let network = network_with_roads(vec![horizontal_road("long", 32.0, 128.0, 700.0, RoadStyleId::DirtRoad)]);
    let chunk_id = ChunkId::new(ChunkCoord::new(1, 0));
    let mut store = RoadDeformationStore::default();
    rebake_single(&world, &mut store, &network, chunk_id);
    let tile = store.tiles.get(&chunk_id).expect("tile");
    let (min_delta, _) = tile_delta_range(tile);
    assert!(min_delta > -2.0);
}

#[test]
fn two_separated_roads_do_not_corrupt_each_other() {
    let mut world = WorldData::new(layout());
    world.insert(ChunkId::new(ChunkCoord::new(0, 0)), flat_chunk(40.0));
    world.insert(ChunkId::new(ChunkCoord::new(4, 0)), flat_chunk(60.0));
    let network = network_with_roads(vec![
        horizontal_road("west", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad),
        horizontal_road("east", 64.0 + 256.0 * 4.0, 128.0, 192.0 + 256.0 * 4.0, RoadStyleId::DirtRoad),
    ]);
    let dirty = affected_chunk_ids_for_network(&network, layout());
    let mut store = RoadDeformationStore::default();
    rebake(&world, &mut store, &network, &dirty);
    let west_tile = store
        .tiles
        .get(&ChunkId::new(ChunkCoord::new(0, 0)))
        .expect("west tile");
    let east_tile = store
        .tiles
        .get(&ChunkId::new(ChunkCoord::new(4, 0)))
        .expect("east tile");
    let (west_min, _) = tile_delta_range(west_tile);
    let (east_min, _) = tile_delta_range(east_tile);
    assert!(west_min > -2.0);
    assert!(east_min > -2.0);
    assert_eq!(
        store.tiles.get(&ChunkId::new(ChunkCoord::new(2, 0))),
        None
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
    sync_store(&store, &mut world, chunk_id);

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
    sync_store(&store, &mut world, chunk_id);

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
fn chunk_border_heights_match_for_road_across_chunks() {
    let mut world = WorldData::new(layout());
    world.insert(ChunkId::new(ChunkCoord::new(0, 0)), flat_chunk(2.0));
    world.insert(ChunkId::new(ChunkCoord::new(1, 0)), flat_chunk(2.0));
    let network = network_with_roads(vec![horizontal_road("r1", 200.0, 128.0, 300.0, RoadStyleId::DirtRoad)]);
    let mut store = RoadDeformationStore::default();
    let dirty = affected_chunk_ids_for_network(&network, layout());
    rebake(&world, &mut store, &network, &dirty);
    sync_all(&store, &mut world, &dirty);

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
    sync_store(&store, &mut world, chunk_id);
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
    sync_store(&store, &mut world, chunk_id);
    assert!(
        (try_sample_height_at_position(&world, center).unwrap()
            - try_sample_base_height_at_position(&world, center).unwrap())
            .abs()
            < 1e-4
    );
}

#[test]
fn real_world_lazy_rebake_deltas_stay_bounded() {
    use crate::terrain::TerrainWorldCatalog;
    use crate::world::WorldConfig;
    use crate::world::road::load_road_network_from_world_package;
    use std::path::Path;

    let world_dir = Path::new("assets/worlds/main");
    let network = load_road_network_from_world_package(world_dir).expect("network");
    let config = WorldConfig::default();
    let catalog =
        TerrainWorldCatalog::from_manifest(&world_dir.join("manifest.ron"), &config).expect("catalog");
    let layout = config.chunk_layout();
    let affected = affected_chunk_ids_for_network(&network, layout);
    assert!(!affected.is_empty(), "expected roads in authored network");

    let world = WorldData::new(layout);
    let mut store = RoadDeformationStore::default();
    rebake_road_deformation_for_chunks(
        &world,
        &mut store,
        &network,
        layout,
        &affected,
        Some(&catalog),
    );

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut baked_chunks = 0usize;
    for chunk_id in affected {
        if let Some(tile) = store.tiles.get(&chunk_id) {
            baked_chunks += 1;
            for delta in &tile.deltas {
                if delta.is_finite() && delta.abs() > 1e-5 {
                    min = min.min(*delta);
                    max = max.max(*delta);
                }
            }
        }
    }
    assert!(baked_chunks > 0, "no catalog-backed chunks baked");
    assert!(
        min > -0.01,
        "catastrophic min delta {} across {} chunks",
        min,
        baked_chunks
    );
    assert!(
        max < 0.01,
        "catastrophic max delta {} across {} chunks",
        max,
        baked_chunks
    );
}

#[test]
fn gaea_scale_flat_road_depression_is_subtle() {
    let base = 0.00238;
    let heightfield =
        Heightfield::from_samples(5, 64.0, vec![base; 25]).unwrap();
    let chunk = ChunkData::new(heightfield, Vec::new());
    let mut world = world_with_chunk(ChunkCoord::new(0, 0), chunk);
    let network =
        network_with_roads(vec![horizontal_road("r1", 64.0, 128.0, 192.0, RoadStyleId::DirtRoad)]);
    let depression_m = network
        .style_defaults(RoadStyleId::DirtRoad)
        .expect("style")
        .depression_m;
    let mut store = RoadDeformationStore::default();
    let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
    rebake_single(&world, &mut store, &network, chunk_id);
    let tile = store.tiles.get(&chunk_id).expect("tile");
    let delta = tile.sample_delta(128.0, 128.0).unwrap();
    let depression_hf = depression_meters_to_heightfield_units(depression_m, base);
    assert!(delta < 0.0);
    assert!(delta.abs() < depression_hf * 2.0);
    assert!(delta.abs() < base * 0.05, "delta {} vs base {}", delta, base);
}

fn sync_store(store: &RoadDeformationStore, world: &mut WorldData, chunk_id: ChunkId) {
    crate::world::road::deformation::sync_store_tiles_to_chunks(
        store,
        world,
        &HashSet::from([chunk_id]),
    );
}

fn sync_all(store: &RoadDeformationStore, world: &mut WorldData, chunk_ids: &HashSet<ChunkId>) {
    crate::world::road::deformation::sync_store_tiles_to_chunks(store, world, chunk_ids);
}
