//! NAV-OPENING-1 regressions: opening-aware Interior agent clearance.

use std::path::Path;

use bevy::prelude::*;

use super::catalog::{
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, BuildingNavigationBlueprintCatalog,
};
use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationPolygon2d, NavigationRegionDefinition,
};
use super::opening_geometry::{
    authored_opening_interval_on_edge, min_interior_closed_boundary_clearance_meters,
    usable_center_opening_interval_on_edge,
};
use super::runtime::{interior_agent_fits_region, min_edge_clearance_meters};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition,
    BuildingDefinitionId, BuildingLifecycleState, BuildingNavigationBlueprintInstanceOverride,
    BuildingOwnership, BuildingRenderKey, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
    DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, LocalPosition, PassabilityAgent,
    PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, WorldData, WorldPosition,
    place_player_building, query_navigation_point_legality, set_building_lifecycle_stage,
};

fn layout_world() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn imported_hut_catalog() -> BuildingCatalog {
    BuildingCatalog::from_definitions(
        vec![BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Survival Hut",
            crate::world::BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            250,
            45.0,
            crate::world::FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )],
        &BuildingCategoryCatalog::default(),
    )
    .expect("hut catalog")
}

fn persisted_nav_catalog() -> BuildingNavigationBlueprintCatalog {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH);
    BuildingNavigationBlueprintCatalog::load_from_ron_path(&path).expect("nav catalog")
}

fn activate_hut(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = crate::world::OccupancyCatalogs {
        building: building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();
    let building_id = place_player_building(
        building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        placement,
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;
    set_building_lifecycle_stage(
        world,
        building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(nav_catalog),
        building_id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .expect("complete hut");
    building_id
}

fn activate_fixture(
    world: &mut WorldData,
    blueprint: BuildingNavigationBlueprint,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let building_catalog = BuildingCatalog::default();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = crate::world::OccupancyCatalogs {
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

fn oversized_concave_hut_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("oversized_concave_hut", "Oversized Concave Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 1.27,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "main".to_string(),
                display_label: "Main".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![
                        [0.0, 0.0],
                        [14.0, 0.0],
                        [14.0, 14.0],
                        [6.0, 14.0],
                        [6.0, 6.0],
                        [0.0, 6.0],
                    ],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [7.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [7.0, 1.27, 1.5],
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

#[test]
fn hut_nav_t3_class_landing_is_legal_for_robot() {
    let mut world = layout_world();
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_hut(&mut world, &building_catalog, &nav_catalog, pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime
        .regions
        .iter()
        .find(|region| region.region_key == "region_3")
        .unwrap()
        .space_id;
    let portal = world
        .space_registry()
        .get_portal(*runtime.portal_keys.get("entrance").unwrap())
        .unwrap();
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .unwrap()
        .floor_y_global;
    let landing_global = portal.to_position.to_global(layout);
    let landing = WorldPosition::from_global(
        Vec3::new(landing_global.x, floor_y, landing_global.z),
        layout,
    );
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let edge_index = portal.entrance_owning_edge_index.unwrap() as usize;
    let landing_xz = landing.to_global(layout).xz();
    let old_clearance = min_edge_clearance_meters(landing_xz, &region.world_outline_xz);
    let opening_clearance = min_interior_closed_boundary_clearance_meters(
        landing_xz,
        &region.world_outline_xz,
        world.space_registry(),
        building_id,
        interior_space,
        0.68,
    );
    assert!(
        old_clearance < 0.68,
        "legacy clearance {old_clearance} should be tighter than robot radius"
    );
    assert!(
        opening_clearance >= 0.68,
        "opening-aware clearance {opening_clearance} should satisfy robot radius"
    );

    let result = query_navigation_point_legality(
        &world,
        PassabilityCatalogs {
            doodad: &DoodadCatalog::default(),
            building: &BuildingCatalog::default(),
            footprint: &FootprintCatalog::default(),
        },
        landing,
        PassabilityAgent {
            radius_meters: 0.68,
            max_slope_degrees: 40.0,
        },
        interior_space,
    );
    assert!(
        matches!(result, PassabilityResult::Passable { .. }),
        "T3-class landing should be interior point-legal: {result:?}"
    );
}

#[test]
fn closed_edge_still_requires_clearance() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        oversized_concave_hut_blueprint(),
        pos(80.0, 80.0),
    );
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime.regions[0].space_id;
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .unwrap()
        .floor_y_global;
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let (inward, _) = edge_outward_inward(&region.world_outline_xz, 1);
    let near_closed = WorldPosition::from_global(
        Vec3::new(
            region.world_outline_xz[1].x + inward.x * 0.5,
            floor_y,
            region.world_outline_xz[1].y + inward.y * 0.5,
        ),
        layout,
    );
    let agent = 0.68;
    assert!(!interior_agent_fits_region(
        world.building_navigation_runtime(),
        world.space_registry(),
        layout,
        near_closed,
        interior_space,
        agent,
    ));
}

#[test]
fn opening_endpoint_enforces_agent_clearance() {
    let mut world = layout_world();
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_hut(&mut world, &building_catalog, &nav_catalog, pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime
        .regions
        .iter()
        .find(|r| r.region_key == "region_3")
        .unwrap()
        .space_id;
    let portal = world
        .space_registry()
        .get_portal(*runtime.portal_keys.get("entrance").unwrap())
        .unwrap();
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .unwrap()
        .floor_y_global;
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let edge_index = portal.entrance_owning_edge_index.unwrap() as usize;
    let a = region.world_outline_xz[edge_index];
    let b = region.world_outline_xz[(edge_index + 1) % region.world_outline_xz.len()];
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let usable =
        usable_center_opening_interval_on_edge(a, b, threshold, portal.from_radius_meters, 0.68)
            .expect("usable interval");
    let edge = b - a;
    let endpoint = a + edge * usable.t_start;
    let (inward, _) = edge_outward_inward(&region.world_outline_xz, edge_index);
    let near_endpoint = WorldPosition::from_global(
        Vec3::new(
            endpoint.x + inward.x * 0.69,
            floor_y,
            endpoint.y + inward.y * 0.69,
        ),
        layout,
    );
    assert!(!interior_agent_fits_region(
        world.building_navigation_runtime(),
        world.space_registry(),
        layout,
        near_endpoint,
        interior_space,
        0.68,
    ));
}

#[test]
fn wide_opening_allows_off_center_approach() {
    let mut world = layout_world();
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_hut(&mut world, &building_catalog, &nav_catalog, pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime
        .regions
        .iter()
        .find(|r| r.region_key == "region_3")
        .unwrap()
        .space_id;
    let portal = world
        .space_registry()
        .get_portal(*runtime.portal_keys.get("entrance").unwrap())
        .unwrap();
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .unwrap()
        .floor_y_global;
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let edge_index = portal.entrance_owning_edge_index.unwrap() as usize;
    let a = region.world_outline_xz[edge_index];
    let b = region.world_outline_xz[(edge_index + 1) % region.world_outline_xz.len()];
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let usable =
        usable_center_opening_interval_on_edge(a, b, threshold, portal.from_radius_meters, 0.68)
            .expect("usable interval");
    let edge = b - a;
    let edge_len = edge.length();
    let tangential = if edge_len > f32::EPSILON {
        edge / edge_len
    } else {
        Vec2::X
    };
    let t_thresh = ((threshold - a).dot(edge) / edge.length_squared()).clamp(0.0, 1.0);
    // Lateral offset within usable interval (not at opening center).
    let lateral_t = usable.t_start + (usable.t_end - usable.t_start) * 0.25;
    assert!(
        lateral_t > t_thresh + 0.01 || lateral_t < t_thresh - 0.01,
        "lateral sample must be off-center within usable interval"
    );
    let lateral_shift = (lateral_t - t_thresh) * edge_len;
    let landing_xz = portal.to_position.to_global(layout).xz();
    let off_center = landing_xz + tangential * lateral_shift;
    let position =
        WorldPosition::from_global(Vec3::new(off_center.x, floor_y, off_center.y), layout);
    let clearance = min_interior_closed_boundary_clearance_meters(
        off_center,
        &region.world_outline_xz,
        world.space_registry(),
        building_id,
        interior_space,
        0.68,
    );
    assert!(
        clearance >= 0.68,
        "off-center opening clearance {clearance} should satisfy robot radius"
    );
    assert!(interior_agent_fits_region(
        world.building_navigation_runtime(),
        world.space_registry(),
        layout,
        position,
        interior_space,
        0.68,
    ));
}

#[test]
fn too_large_agent_has_no_usable_opening_interval() {
    let a = Vec2::new(0.0, 0.0);
    let b = Vec2::new(10.0, 0.0);
    let threshold = Vec2::new(5.0, 0.0);
    assert!(usable_center_opening_interval_on_edge(a, b, threshold, 0.5, 0.68).is_none());
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, narrow_entrance_blueprint(), pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime.regions[0].space_id;
    let portal = world
        .space_registry()
        .get_portal(*runtime.portal_keys.get("exterior_entrance").unwrap())
        .unwrap();
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
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
        interior_space,
        0.68,
    ));
}

#[test]
fn concave_region_authority_preserved() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        oversized_concave_hut_blueprint(),
        pos(80.0, 80.0),
    );
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime.regions[0].space_id;
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .unwrap()
        .floor_y_global;
    let notch = WorldPosition::from_global(Vec3::new(5.0, floor_y, 10.0), layout);
    assert!(!interior_agent_fits_region(
        world.building_navigation_runtime(),
        world.space_registry(),
        layout,
        notch,
        interior_space,
        0.6,
    ));
}

#[test]
fn disabled_portal_treats_edge_as_closed() {
    let mut world = layout_world();
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_hut(&mut world, &building_catalog, &nav_catalog, pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let interior_space = runtime
        .regions
        .iter()
        .find(|r| r.region_key == "region_3")
        .unwrap()
        .space_id;
    let portal_id = *runtime.portal_keys.get("entrance").unwrap();
    world
        .space_registry_mut()
        .get_portal_mut(portal_id)
        .unwrap()
        .enabled = false;
    let portal = world.space_registry().get_portal(portal_id).unwrap();
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
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
        interior_space,
        0.68,
    ));
}

#[test]
fn point_and_segment_share_usable_opening_interval() {
    let a = Vec2::new(0.0, 0.0);
    let b = Vec2::new(20.0, 0.0);
    let threshold = Vec2::new(10.0, 0.0);
    let half_width = 4.0;
    let agent_radius = 0.68;
    let usable = usable_center_opening_interval_on_edge(a, b, threshold, half_width, agent_radius)
        .expect("usable interval");
    let intersection = Vec2::new(12.0, 0.0);
    let t = ((intersection - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
    assert!(t >= usable.t_start && t <= usable.t_end);
    assert!(
        super::opening_geometry::point_within_usable_center_opening_on_edge(
            intersection,
            a,
            b,
            threshold,
            half_width,
            agent_radius,
        )
    );
}
