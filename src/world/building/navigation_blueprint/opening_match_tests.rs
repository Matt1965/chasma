//! IN-11gI-A entrance opening match regressions (real `hut_nav` geometry).

use std::path::Path;

use bevy::prelude::*;

use super::ENTRANCE_BOUNDARY_TOLERANCE;
use super::catalog::{
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, BuildingNavigationBlueprintCatalog,
};
use super::id::BuildingNavigationBlueprintId;
use super::runtime::{
    probe_segment_crosses_entrance_opening, surface_segment_respects_blueprint_boundaries,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition,
    BuildingDefinitionId, BuildingLifecycleState, BuildingOwnership, BuildingRenderKey, ChunkCoord,
    ChunkData, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog,
    LocalPosition, NavigationAgent, NavigationConfig, NavigationSegmentLegality, OccupancyCatalogs,
    PassabilityCatalogs, SpaceId, WorldData, WorldPosition, place_player_building,
    query_navigation_segment_legality, set_building_lifecycle_stage,
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
    let occupancy = OccupancyCatalogs {
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
    let mut area = 0.0;
    for index in 0..polygon.len() {
        let vi = polygon[index];
        let vj = polygon[(index + 1) % polygon.len()];
        area += vi.x * vj.y - vj.x * vi.y;
    }
    let inward = if area >= 0.0 { left } else { -left };
    (inward, -inward)
}

const ROBOT_AGENT_RADIUS: f32 = 0.68;

fn segment_legality(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    from: WorldPosition,
    to: WorldPosition,
) -> NavigationSegmentLegality {
    query_navigation_segment_legality(
        world,
        world.space_registry(),
        catalogs,
        NavigationConfig::default(),
        SpaceId::SURFACE,
        NavigationAgent {
            radius_meters: 0.68,
            max_slope_degrees: 45.0,
        },
        from,
        to,
        world.layout(),
    )
}

#[test]
fn hut_nav_entrance_threshold_on_owning_edge() {
    let mut world = layout_world();
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let blueprint = nav_catalog
        .get(&BuildingNavigationBlueprintId::new("hut_nav"))
        .expect("hut_nav");
    assert!(
        blueprint.entrances.iter().any(|e| e.key == "entrance"),
        "catalog must contain entrance"
    );
    let building_id = activate_hut(&mut world, &building_catalog, &nav_catalog, pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let interior_space = runtime
        .regions
        .iter()
        .find(|region| region.region_key == "region_3")
        .expect("region_3")
        .space_id;
    let portal_id = runtime
        .portal_keys
        .get("entrance")
        .expect("entrance portal");
    let portal = world
        .space_registry()
        .get_portal(*portal_id)
        .expect("portal record");
    assert_eq!(portal.entrance_owning_edge_index, Some(4));
    let threshold = portal
        .entrance_threshold_global_xz
        .expect("threshold metadata");
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .expect("region");
    let a = region.world_outline_xz[4];
    let b = region.world_outline_xz[5];
    let edge = b - a;
    let len_sq = edge.length_squared();
    let t = ((threshold - a).dot(edge) / len_sq).clamp(0.0, 1.0);
    let on_edge = a + edge * t;
    assert!(
        on_edge.distance(threshold) <= ENTRANCE_BOUNDARY_TOLERANCE,
        "threshold must lie on owning edge within tolerance (dist={})",
        on_edge.distance(threshold)
    );
    assert!(
        on_edge.distance(threshold) < 0.1,
        "threshold must be on-edge after floor-elevation transform, not offset like staging/landing midpoint"
    );
    let staging = portal.from_center_global_xz;
    let landing = portal.to_position.to_global(world.layout()).xz();
    let (inward, outward) = edge_outward_inward(&region.world_outline_xz, 4);
    assert!(
        outward.dot(staging - threshold) > 0.0,
        "staging must be exterior"
    );
    assert!(
        inward.dot(landing - threshold) > 0.0,
        "landing must be interior"
    );
}

#[test]
fn hut_nav_segment_inside_opening_matches_entrance() {
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
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let (inward, outward) = edge_outward_inward(&region.world_outline_xz, 4);
    let from_xz = threshold + outward * 3.0;
    let to_xz = threshold + inward * 3.0;
    let from = pos(from_xz.x, from_xz.y);
    let to = pos(to_xz.x, to_xz.y);
    assert!(
        probe_segment_crosses_entrance_opening(&world, from, to, ROBOT_AGENT_RADIUS),
        "segment through opening must match entrance"
    );
    assert!(
        surface_segment_respects_blueprint_boundaries(
            &world,
            from,
            to,
            world.layout(),
            ROBOT_AGENT_RADIUS,
        ),
        "boundary crossing through opening must be permitted"
    );
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    assert!(
        segment_legality(&world, catalogs, from, to).is_legal(),
        "universal segment legality must permit opening crossing"
    );
}

#[test]
fn hut_nav_segment_outside_opening_on_same_edge_rejected() {
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
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let a = region.world_outline_xz[4];
    let b = region.world_outline_xz[5];
    let edge = (b - a).normalize();
    let far_on_edge = threshold + edge * (portal.from_radius_meters + 3.0);
    let (inward, outward) = edge_outward_inward(&region.world_outline_xz, 4);
    let from = pos(
        (far_on_edge + outward * 3.0).x,
        (far_on_edge + outward * 3.0).y,
    );
    let to = pos(
        (far_on_edge + inward * 3.0).x,
        (far_on_edge + inward * 3.0).y,
    );
    assert!(
        !probe_segment_crosses_entrance_opening(&world, from, to, ROBOT_AGENT_RADIUS),
        "crossing outside opening span must not match"
    );
    assert!(
        !surface_segment_respects_blueprint_boundaries(
            &world,
            from,
            to,
            world.layout(),
            ROBOT_AGENT_RADIUS,
        ),
        "closed boundary segment must be rejected"
    );
}

#[test]
fn hut_nav_edge_four_entrance_does_not_exempt_other_edges() {
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
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let edge_index = 6;
    let a = region.world_outline_xz[edge_index];
    let b = region.world_outline_xz[(edge_index + 1) % region.world_outline_xz.len()];
    let mid = a + (b - a) * 0.5;
    let (inward, outward) = edge_outward_inward(&region.world_outline_xz, edge_index);
    let from = pos((mid + outward * 1.5).x, (mid + outward * 1.5).y);
    let to = pos((mid + inward * 1.5).x, (mid + inward * 1.5).y);
    assert!(
        !probe_segment_crosses_entrance_opening(&world, from, to, ROBOT_AGENT_RADIUS),
        "edge-4 entrance must not exempt other edges"
    );
    assert!(
        !surface_segment_respects_blueprint_boundaries(
            &world,
            from,
            to,
            world.layout(),
            ROBOT_AGENT_RADIUS,
        ),
        "crossing a different closed edge must remain illegal"
    );
}

#[test]
fn hut_nav_large_opening_respects_owning_edge_only() {
    let mut world = layout_world();
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_hut(&mut world, &building_catalog, &nav_catalog, pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let portal = world
        .space_registry()
        .get_portal(*runtime.portal_keys.get("entrance").unwrap())
        .unwrap();
    assert!(
        portal.from_radius_meters > 5.0,
        "manual hut entrance must be oversized for this regression"
    );
    let interior_space = runtime
        .regions
        .iter()
        .find(|region| region.region_key == "region_3")
        .unwrap()
        .space_id;
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let edge_len = region.world_outline_xz[4].distance(region.world_outline_xz[5]);
    assert!(
        portal.from_radius_meters * 2.0 > edge_len * 0.5,
        "opening spans most of owning edge but must still be edge-local"
    );
    let (inward, outward) = edge_outward_inward(&region.world_outline_xz, 4);
    let from = pos((threshold + outward * 2.0).x, (threshold + outward * 2.0).y);
    let to = pos((threshold + inward * 2.0).x, (threshold + inward * 2.0).y);
    assert!(probe_segment_crosses_entrance_opening(
        &world,
        from,
        to,
        ROBOT_AGENT_RADIUS
    ));
}

#[test]
fn hut_nav_segment_crossing_inside_opening_off_centerline() {
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
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .unwrap();
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let a = region.world_outline_xz[4];
    let b = region.world_outline_xz[5];
    let edge = (b - a).normalize();
    // Crossing on edge 4 away from threshold centerline but still inside the oversized opening.
    let cross_on_edge = threshold + edge * 0.5;
    let (inward, outward) = edge_outward_inward(&region.world_outline_xz, 4);
    let from = pos(
        (cross_on_edge + outward * 2.0).x,
        (cross_on_edge + outward * 2.0).y,
    );
    let to = pos(
        (cross_on_edge + inward * 2.0).x,
        (cross_on_edge + inward * 2.0).y,
    );
    assert!(
        probe_segment_crosses_entrance_opening(&world, from, to, ROBOT_AGENT_RADIUS),
        "off-centerline crossing inside opening must match"
    );
    assert!(
        surface_segment_respects_blueprint_boundaries(
            &world,
            from,
            to,
            world.layout(),
            ROBOT_AGENT_RADIUS,
        ),
        "off-centerline opening crossing must permit boundary traversal"
    );
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    assert!(
        segment_legality(&world, catalogs, from, to).is_legal(),
        "universal segment legality must permit off-centerline opening crossing"
    );
}
