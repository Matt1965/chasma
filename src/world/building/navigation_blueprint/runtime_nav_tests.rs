//! NV2 interior navigation runtime integration tests.

use bevy::prelude::*;

use super::runtime::{
    interior_position_walkable, resolve_navigation_start_space,
};
use crate::world::unit::{UnitOrder, UnitSource, UnitState, create_unit, step_unit_movement};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState, BuildingOwnership,
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog,
    Heightfield, LocalPosition, NavigationConfig, OccupancyCatalogs, PassabilityCatalogs,
    SpaceId, UnitDefinitionId, WorldData, WorldPosition, find_path_with_spaces,
    place_player_building, resolve_pending_unit_orders, set_building_lifecycle_stage,
    BuildingNavigationBlueprintCatalog,
};

fn layout_world() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
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

fn occ<'a>(
    building: &'a BuildingCatalog,
    doodad: &'a DoodadCatalog,
    footprint: &'a FootprintCatalog,
) -> OccupancyCatalogs<'a> {
    OccupancyCatalogs {
        building,
        doodad,
        footprint,
    }
}

fn activate_hut(world: &mut WorldData) -> crate::world::BuildingId {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = crate::world::InteriorProfileCatalog::default();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();

    let id = place_player_building(
        &building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .unwrap()
    .id;

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

fn hut_ground_target(world: &WorldData, building_id: crate::world::BuildingId) -> (SpaceId, WorldPosition) {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let ground = runtime
        .floors
        .iter()
        .find(|floor| floor.floor_key == "ground_interior")
        .expect("ground floor");
    let layout = world.layout();
    let position = interior_centroid(&ground.world_outline_xz, layout, ground.space_id, world);
    (ground.space_id, position)
}

fn hut_entrance_approach(world: &WorldData, building_id: crate::world::BuildingId) -> WorldPosition {
    let layout = world.layout();
    let portal_id = world
        .space_registry()
        .portals()
        .find(|(_, portal)| {
            portal.portal_type == crate::world::PortalType::ExteriorEntrance
                && portal.owning_building_id == Some(building_id)
        })
        .map(|(id, _)| *id)
        .expect("entrance portal");
    let portal = world.space_registry().get_portal(portal_id).unwrap();
    let building = world.get_building(building_id).unwrap();
    let building_center = building.placement.position.to_global(layout);
    let portal_xz = portal.from_center_global_xz;
    let away = Vec2::new(portal_xz.x, portal_xz.y) - Vec2::new(building_center.x, building_center.z);
    let away = if away.length_squared() > 1e-4 {
        away.normalize()
    } else {
        Vec2::new(1.0, 0.0)
    };
    let approach = portal_xz - away * 4.0;
    WorldPosition::from_global(Vec3::new(approach.x, 0.0, approach.y), layout)
}

fn interior_centroid(
    outline: &[Vec2],
    layout: ChunkLayout,
    space_id: SpaceId,
    world: &WorldData,
) -> WorldPosition {
    let centroid_xz = outline.iter().fold(Vec2::ZERO, |acc, v| acc + *v) / outline.len() as f32;
    let floor_y = world
        .space_registry()
        .get_space(space_id)
        .map(|space| space.floor_y_global)
        .unwrap_or(0.0);
    WorldPosition::from_global(Vec3::new(centroid_xz.x, floor_y, centroid_xz.y), layout)
}

#[test]
fn start_space_resolves_from_interior_position() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world);
    let (ground_space_id, interior_pos) = hut_ground_target(&world, building_id);
    let resolved = resolve_navigation_start_space(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        interior_pos,
        SpaceId::SURFACE,
    );
    assert_eq!(resolved, ground_space_id);
}

#[test]
fn interior_outside_blueprint_floor_is_blocked() {
    let mut world = layout_world();
    let _ = activate_hut(&mut world);
    let runtime = world.building_navigation_runtime();
    let ground = runtime
        .iter()
        .flat_map(|entry| entry.floors.iter())
        .find(|floor| floor.floor_key == "ground_interior")
        .expect("ground floor");
    let layout = world.layout();
    let outside = pos(40.0, 40.0);
    assert!(!interior_position_walkable(
        runtime,
        world.space_registry(),
        layout,
        outside,
        ground.space_id,
    ));
}

#[test]
fn surface_to_hut_interior_path_uses_entrance_portal() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world);
    let (ground_space_id, interior_goal) = hut_ground_target(&world, building_id);
    let start = hut_entrance_approach(&world, building_id);

    let catalogs = PassabilityCatalogs {
        doodad: &DoodadCatalog::default(),
        building: &BuildingCatalog::default(),
        footprint: &FootprintCatalog::default(),
    };
    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        0.5,
        45.0,
        start,
        interior_goal,
        SpaceId::SURFACE,
        ground_space_id,
        None,
    )
    .expect("surface to interior path");
    assert!(path.waypoints.iter().any(|wp| wp.portal_id.is_some()));
    assert!(path
        .waypoints
        .iter()
        .any(|wp| wp.space_id == ground_space_id));
}

#[test]
fn unit_enters_hut_interior_through_entrance() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();

    let building_id = activate_hut(&mut world);
    let ground_space_id = hut_ground_target(&world, building_id).0;
    let interior_goal = hut_ground_target(&world, building_id).1;
    let start = hut_entrance_approach(&world, building_id);

    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
    )
    .unwrap()
    .id;

    world.command_buffer_mut().enqueue(
        unit_id,
        UnitOrder::MoveTo {
            target: interior_goal,
        },
    );
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let resolve_report = resolve_pending_unit_orders(
        &mut world,
        &unit_catalog,
        catalogs,
        &NavigationConfig::default(),
    );
    assert_eq!(resolve_report.resolved, 1, "move order should resolve");
    let unit = world.get_unit(unit_id).unwrap();
    let UnitState::Moving { ref path, .. } = unit.state else {
        panic!("unit should be moving after path resolve");
    };
    assert!(
        path.waypoints.iter().any(|wp| wp.portal_id.is_some()),
        "path should include a portal transition"
    );

    let layout = world.layout();
    let goal_xz = interior_goal.to_global(layout).xz();
    let mut reached_interior = false;
    for _ in 0..200 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        let pos_xz = record.placement.position.to_global(layout).xz();
        if record.current_space_id == ground_space_id || pos_xz.distance(goal_xz) < 2.0 {
            reached_interior = true;
            break;
        }
    }
    assert!(
        reached_interior,
        "unit should reach hut interior via blueprint navigation"
    );
}
