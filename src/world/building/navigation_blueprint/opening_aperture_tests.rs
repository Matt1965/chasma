//! NAV-APERTURE regressions: agent-usable opening crossing matches point clearance.

use bevy::prelude::*;

use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationPolygon2d, NavigationRegionDefinition,
};
use super::opening_geometry::{
    point_within_authored_opening_on_edge, point_within_usable_center_opening_on_edge,
    usable_center_opening_interval_on_edge,
};
use super::runtime::{
    interior_agent_fits_region, probe_segment_crosses_entrance_opening,
    surface_segment_respects_blueprint_boundaries,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingNavigationBlueprintInstanceOverride, BuildingOwnership, ChunkCoord, ChunkLayout,
    DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, LocalPosition, OccupancyCatalogs,
    SpaceId, WorldData, WorldPosition, place_player_building, set_building_lifecycle_stage,
};

const ROBOT_RADIUS: f32 = 0.68;

fn layout_world() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        crate::world::ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn wide_entrance_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("wide_entrance_hut", "Wide Entrance Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "main".to_string(),
                display_label: "Main".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 4.0], [0.0, 4.0]],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "wide_test_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [5.0, 0.0],
            radius_meters: 2.0,
            interior_spawn_local: [5.0, 0.0, 0.8],
            bidirectional: true,
            door_key: None,
        }])
}

fn narrow_entrance_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("narrow_entrance_hut", "Narrow Entrance Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "main".to_string(),
                display_label: "Main".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![[0.0, 0.0], [12.0, 0.0], [12.0, 8.0], [0.0, 8.0]],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [6.0, 0.0],
            radius_meters: 0.4,
            interior_spawn_local: [6.0, 0.0, 0.8],
            bidirectional: true,
            door_key: None,
        }])
}

fn activate_fixture(
    world: &mut WorldData,
    blueprint: BuildingNavigationBlueprint,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let building_catalog = BuildingCatalog::default();
    let nav_catalog = crate::world::BuildingNavigationBlueprintCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();
    let id = place_player_building(
        &building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        placement,
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .unwrap()
    .id;
    world
        .mutate_building(id, |record| {
            record.interior.navigation_blueprint_override = Some(
                BuildingNavigationBlueprintInstanceOverride::inline(blueprint),
            );
        })
        .expect("building");
    set_building_lifecycle_stage(
        world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(&nav_catalog),
        id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .unwrap();
    id
}

fn edge_outward_inward(polygon: &[Vec2], edge_index: usize) -> (Vec2, Vec2) {
    let a = polygon[edge_index];
    let b = polygon[(edge_index + 1) % polygon.len()];
    let edge = b - a;
    let len = edge.length();
    let left = if len > f32::EPSILON {
        Vec2::new(-edge.y, edge.x) / len
    } else {
        Vec2::Y
    };
    let centroid =
        polygon.iter().copied().fold(Vec2::ZERO, |acc, p| acc + p) / polygon.len().max(1) as f32;
    let mid = (a + b) * 0.5;
    let inward = if left.dot(centroid - mid) > 0.0 {
        left
    } else {
        -left
    };
    (-inward, inward)
}

struct WideEntranceFixture {
    building_id: crate::world::BuildingId,
    interior_space: SpaceId,
    edge_index: usize,
    threshold: Vec2,
    edge_a: Vec2,
    edge_b: Vec2,
    opening_half_width: f32,
}

fn entrance_fixture(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    portal_key: &str,
) -> WideEntranceFixture {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime.regions[0].space_id;
    let portal = world
        .space_registry()
        .get_portal(*runtime.portal_keys.get(portal_key).unwrap())
        .unwrap();
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let edge_index = portal.entrance_owning_edge_index.unwrap() as usize;
    WideEntranceFixture {
        building_id,
        interior_space,
        edge_index,
        threshold: portal.entrance_threshold_global_xz.unwrap(),
        edge_a: region.world_outline_xz[edge_index],
        edge_b: region.world_outline_xz[(edge_index + 1) % region.world_outline_xz.len()],
        opening_half_width: portal.from_radius_meters,
    }
}

fn offset_meters_from_threshold(fixture: &WideEntranceFixture, parametric_t: f32) -> f32 {
    let edge = fixture.edge_b - fixture.edge_a;
    let edge_len = edge.length();
    let t_thresh =
        ((fixture.threshold - fixture.edge_a).dot(edge) / edge_len.powi(2)).clamp(0.0, 1.0);
    (parametric_t - t_thresh) * edge_len
}

fn usable_interval(
    fixture: &WideEntranceFixture,
) -> super::opening_geometry::EdgeParametricInterval {
    usable_center_opening_interval_on_edge(
        fixture.edge_a,
        fixture.edge_b,
        fixture.threshold,
        fixture.opening_half_width,
        ROBOT_RADIUS,
    )
    .expect("usable interval")
}

fn crossing_segment(
    world: &WorldData,
    fixture: &WideEntranceFixture,
    offset_along_edge_meters: f32,
) -> (WorldPosition, WorldPosition) {
    let region = world
        .building_navigation_runtime()
        .region_for_space(fixture.interior_space)
        .unwrap();
    let edge = (fixture.edge_b - fixture.edge_a).normalize();
    let cross_on_edge = fixture.threshold + edge * offset_along_edge_meters;
    let (inward, outward) = edge_outward_inward(&region.world_outline_xz, fixture.edge_index);
    let from = pos(
        (cross_on_edge + outward * 3.0).x,
        (cross_on_edge + outward * 3.0).y,
    );
    let to = pos(
        (cross_on_edge + inward * 3.0).x,
        (cross_on_edge + inward * 3.0).y,
    );
    (from, to)
}

#[test]
fn endpoint_crossing_rejected_inside_authored_but_outside_usable_aperture() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, wide_entrance_blueprint(), pos(80.0, 80.0));
    let fixture = entrance_fixture(&world, building_id, "wide_test_entrance");
    let clip_offset =
        offset_meters_from_threshold(&fixture, usable_interval(&fixture).t_end + 0.05);
    let cross = fixture.threshold + (fixture.edge_b - fixture.edge_a).normalize() * clip_offset;
    assert!(
        point_within_authored_opening_on_edge(
            cross,
            fixture.edge_a,
            fixture.edge_b,
            fixture.threshold,
            fixture.opening_half_width,
        ),
        "crossing must remain inside authored opening"
    );
    assert!(
        !point_within_usable_center_opening_on_edge(
            cross,
            fixture.edge_a,
            fixture.edge_b,
            fixture.threshold,
            fixture.opening_half_width,
            ROBOT_RADIUS,
        ),
        "crossing must lie outside usable center aperture"
    );
    let (from, to) = crossing_segment(&world, &fixture, clip_offset);
    assert!(
        !probe_segment_crosses_entrance_opening(&world, from, to, ROBOT_RADIUS),
        "endpoint clip zone must reject segment crossing"
    );
    assert!(
        !surface_segment_respects_blueprint_boundaries(
            &world,
            from,
            to,
            world.layout(),
            ROBOT_RADIUS,
        ),
        "endpoint clip zone must block boundary traversal"
    );
}

#[test]
fn collect_usable_intervals_populated_for_wide_fixture() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, wide_entrance_blueprint(), pos(80.0, 80.0));
    let fixture = entrance_fixture(&world, building_id, "wide_test_entrance");
    let portal_id = *world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap()
        .portal_keys
        .get("wide_test_entrance")
        .unwrap();
    let portal = world.space_registry().get_portal(portal_id).unwrap();
    assert_eq!(portal.owning_building_id, Some(fixture.building_id));
    assert!(portal.enabled, "doorless entrance must remain traversable");
    assert_eq!(
        portal.portal_type,
        crate::world::PortalType::ExteriorEntrance
    );
    assert_eq!(portal.to_space, fixture.interior_space);
    assert_eq!(
        portal.entrance_owning_edge_index,
        Some(fixture.edge_index as u32)
    );
    let direct = usable_center_opening_interval_on_edge(
        fixture.edge_a,
        fixture.edge_b,
        fixture.threshold,
        portal.from_radius_meters,
        ROBOT_RADIUS,
    );
    assert!(
        direct.is_some(),
        "direct usable interval missing: {direct:?}"
    );
    let intervals = super::opening_geometry::collect_usable_entrance_openings_on_edge(
        world.space_registry(),
        fixture.building_id,
        fixture.interior_space,
        fixture.edge_index,
        fixture.edge_a,
        fixture.edge_b,
        ROBOT_RADIUS,
    );
    assert!(
        !intervals.is_empty(),
        "expected usable intervals, got {intervals:?}"
    );
}

#[test]
fn crossing_accepted_just_inside_usable_aperture() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, wide_entrance_blueprint(), pos(80.0, 80.0));
    let fixture = entrance_fixture(&world, building_id, "wide_test_entrance");
    let usable = usable_interval(&fixture);
    let near_endpoint_t = usable.t_start + (usable.t_end - usable.t_start) * 0.9;
    let safe_offset = offset_meters_from_threshold(&fixture, near_endpoint_t);
    let (from, to) = crossing_segment(&world, &fixture, safe_offset);
    assert!(
        probe_segment_crosses_entrance_opening(&world, from, to, ROBOT_RADIUS),
        "usable aperture crossing must match"
    );
}

#[test]
fn wide_opening_center_and_off_center_crossings_use_full_usable_aperture() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, wide_entrance_blueprint(), pos(80.0, 80.0));
    let fixture = entrance_fixture(&world, building_id, "wide_test_entrance");
    let usable = usable_interval(&fixture);
    let center_offset =
        offset_meters_from_threshold(&fixture, (usable.t_start + usable.t_end) * 0.5);
    let center = crossing_segment(&world, &fixture, center_offset);
    assert!(probe_segment_crosses_entrance_opening(
        &world,
        center.0,
        center.1,
        ROBOT_RADIUS,
    ));
    let off_center_offset = offset_meters_from_threshold(
        &fixture,
        usable.t_start + (usable.t_end - usable.t_start) * 0.35,
    );
    let off_center = crossing_segment(&world, &fixture, off_center_offset);
    assert!(probe_segment_crosses_entrance_opening(
        &world,
        off_center.0,
        off_center.1,
        ROBOT_RADIUS,
    ));
    let clip_offset = offset_meters_from_threshold(&fixture, usable.t_end + 0.05);
    let clip_zone = crossing_segment(&world, &fixture, clip_offset);
    assert!(
        !probe_segment_crosses_entrance_opening(&world, clip_zone.0, clip_zone.1, ROBOT_RADIUS),
        "off-center crossing in endpoint-clearance zone must be rejected"
    );
}

#[test]
fn too_large_agent_point_and_segment_both_blocked() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, narrow_entrance_blueprint(), pos(80.0, 80.0));
    let fixture = entrance_fixture(&world, building_id, "exterior_entrance");
    assert!(
        usable_center_opening_interval_on_edge(
            fixture.edge_a,
            fixture.edge_b,
            fixture.threshold,
            fixture.opening_half_width,
            ROBOT_RADIUS,
        )
        .is_none()
    );
    let (from, to) = crossing_segment(&world, &fixture, 0.0);
    assert!(!probe_segment_crosses_entrance_opening(
        &world,
        from,
        to,
        ROBOT_RADIUS
    ));
    assert!(!surface_segment_respects_blueprint_boundaries(
        &world,
        from,
        to,
        world.layout(),
        ROBOT_RADIUS,
    ));
    let portal = world
        .space_registry()
        .get_portal(
            *world
                .building_navigation_runtime()
                .get(building_id)
                .unwrap()
                .portal_keys
                .get("exterior_entrance")
                .unwrap(),
        )
        .unwrap();
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(fixture.interior_space)
        .unwrap()
        .floor_y_global;
    let landing = WorldPosition::from_global(
        Vec3::new(
            portal.to_position.to_global(layout).x,
            floor_y,
            portal.to_position.to_global(layout).z,
        ),
        layout,
    );
    assert!(!interior_agent_fits_region(
        world.building_navigation_runtime(),
        world.space_registry(),
        layout,
        landing,
        fixture.interior_space,
        ROBOT_RADIUS,
    ));
}

#[test]
fn disabled_portal_rejects_usable_crossing() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, wide_entrance_blueprint(), pos(80.0, 80.0));
    let portal_id = *world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap()
        .portal_keys
        .get("wide_test_entrance")
        .unwrap();
    world
        .space_registry_mut()
        .get_portal_mut(portal_id)
        .unwrap()
        .enabled = false;
    let fixture = entrance_fixture(&world, building_id, "wide_test_entrance");
    let usable = usable_interval(&fixture);
    let center_offset =
        offset_meters_from_threshold(&fixture, (usable.t_start + usable.t_end) * 0.5);
    let (from, to) = crossing_segment(&world, &fixture, center_offset);
    assert!(!probe_segment_crosses_entrance_opening(
        &world,
        from,
        to,
        ROBOT_RADIUS
    ));
    assert!(!surface_segment_respects_blueprint_boundaries(
        &world,
        from,
        to,
        world.layout(),
        ROBOT_RADIUS,
    ));
}

#[test]
fn point_and_segment_agree_on_endpoint_clip_zone() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, wide_entrance_blueprint(), pos(80.0, 80.0));
    let fixture = entrance_fixture(&world, building_id, "wide_test_entrance");
    let region = world
        .building_navigation_runtime()
        .region_for_space(fixture.interior_space)
        .unwrap();
    let (inward, _) = edge_outward_inward(&region.world_outline_xz, fixture.edge_index);
    let usable = usable_interval(&fixture);
    let clip_offset = offset_meters_from_threshold(&fixture, usable.t_end + 0.05);
    let edge = (fixture.edge_b - fixture.edge_a).normalize();
    let on_edge = fixture.threshold + edge * clip_offset;
    let interior_point = on_edge + inward * 0.5;
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(fixture.interior_space)
        .unwrap()
        .floor_y_global;
    let position = WorldPosition::from_global(
        Vec3::new(interior_point.x, floor_y, interior_point.y),
        layout,
    );
    assert!(!interior_agent_fits_region(
        world.building_navigation_runtime(),
        world.space_registry(),
        layout,
        position,
        fixture.interior_space,
        ROBOT_RADIUS,
    ));
    let (from, to) = crossing_segment(&world, &fixture, clip_offset);
    assert!(!probe_segment_crosses_entrance_opening(
        &world,
        from,
        to,
        ROBOT_RADIUS
    ));
}
