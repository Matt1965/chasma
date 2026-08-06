//! NV2 interior navigation runtime integration tests.

use bevy::prelude::*;

use super::runtime::{
    interior_position_walkable, resolve_navigation_space_at_position,
    resolve_navigation_start_space,
};
use crate::world::unit::{UnitOrder, UnitSource, UnitState, create_unit, step_unit_movement};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingNavigationBlueprintCatalog, BuildingOwnership, ChunkCoord, ChunkData, ChunkId,
    ChunkLayout, DoodadCatalog, DoorState, FootprintCatalog, Heightfield, LocalPosition,
    NavigationConfig, OccupancyCatalogs, PassabilityCatalogs, SpaceId, UnitDefinitionId, WorldData,
    WorldPosition, close_door, find_path_with_spaces, open_door, place_player_building,
    resolve_pending_unit_orders, set_building_lifecycle_stage,
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

fn hut_building_center(world: &WorldData, building_id: crate::world::BuildingId) -> WorldPosition {
    world
        .get_building(building_id)
        .map(|record| record.placement.position)
        .expect("building")
}

#[test]
fn interior_unit_passes_owning_building_footprint_xz() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world);
    let ground_space_id = hut_ground_target(&world, building_id).0;
    let interior_spawn = hut_interior_spawn(&world, building_id);
    let building_center = hut_building_center(&world, building_id);
    let layout = world.layout();
    let catalogs = PassabilityCatalogs {
        doodad: &DoodadCatalog::default(),
        building: &BuildingCatalog::default(),
        footprint: &FootprintCatalog::default(),
    };
    let agent = crate::world::PassabilityAgent {
        radius_meters: 0.6,
        max_slope_degrees: 45.0,
    };
    assert!(
        !matches!(
            crate::world::query_passability_at(&world, catalogs, building_center, agent),
            crate::world::PassabilityResult::Passable { .. }
        ),
        "surface must block building footprint"
    );
    let spawn_xz = interior_spawn.to_global(layout).xz();
    let surface_at_spawn_xz =
        WorldPosition::from_global(Vec3::new(spawn_xz.x, 0.0, spawn_xz.y), layout);
    assert!(
        !matches!(
            crate::world::query_passability_at(&world, catalogs, surface_at_spawn_xz, agent),
            crate::world::PassabilityResult::Passable { .. }
        ),
        "surface must block footprint XZ under interior spawn"
    );
    assert!(
        matches!(
            crate::world::query_passability_in_space(
                &world,
                catalogs,
                interior_spawn,
                agent,
                ground_space_id,
            ),
            crate::world::PassabilityResult::Passable { .. }
        ),
        "interior must ignore owning building footprint at interior spawn XZ"
    );
}

fn agent_clear_interior_position(
    region: &crate::world::building::navigation_blueprint::runtime::RuntimeNavigationRegion,
    layout: ChunkLayout,
    space_id: SpaceId,
    world: &WorldData,
    agent_radius: f32,
) -> WorldPosition {
    const MARGIN: f32 = 0.1;
    let inset = agent_radius + MARGIN;
    let min = region.world_aabb_min_xz;
    let max = region.world_aabb_max_xz;
    let span = max - min;
    let xz = if span.x > inset * 2.0 && span.y > inset * 2.0 {
        min + Vec2::new(inset, inset)
    } else {
        interior_centroid(&region.world_outline_xz, layout, space_id, world)
            .to_global(layout)
            .xz()
    };
    let floor_y = world
        .space_registry()
        .get_space(space_id)
        .map(|space| space.floor_y_global)
        .unwrap_or(0.0);
    WorldPosition::from_global(Vec3::new(xz.x, floor_y, xz.y), layout)
}

fn hut_ground_target(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> (SpaceId, WorldPosition) {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let region = runtime
        .regions
        .iter()
        .find(|region| region.floor_key == "ground_interior")
        .expect("ground floor");
    let layout = world.layout();
    let position = agent_clear_interior_position(region, layout, region.space_id, world, 0.6);
    (region.space_id, position)
}

fn hut_entrance_approach(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> WorldPosition {
    let layout = world.layout();
    let portal_id = hut_entrance_portal(world, building_id);
    let portal = world.space_registry().get_portal(portal_id).unwrap();
    let portal_xz = portal.from_center_global_xz;
    let approach = portal_xz + Vec2::new(4.0, 0.0);
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
fn surface_unit_overlapping_interior_outline_stays_surface() {
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
    assert_eq!(resolved, SpaceId::SURFACE);
    assert_ne!(resolved, ground_space_id);
}

#[test]
fn interior_outside_blueprint_floor_is_blocked() {
    let mut world = layout_world();
    let _ = activate_hut(&mut world);
    let runtime = world.building_navigation_runtime();
    let ground = runtime
        .iter()
        .flat_map(|entry| entry.regions.iter())
        .find(|region| region.floor_key == "ground_interior")
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
fn hut_path_endpoints_are_walkable_for_default_fixture() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world);
    let (ground_space_id, interior_goal) = hut_ground_target(&world, building_id);
    let start = hut_entrance_approach(&world, building_id);
    let spawn = hut_interior_spawn(&world, building_id);
    let catalogs = PassabilityCatalogs {
        doodad: &DoodadCatalog::default(),
        building: &BuildingCatalog::default(),
        footprint: &FootprintCatalog::default(),
    };
    let agent = crate::world::NavigationAgent {
        radius_meters: 0.5,
        max_slope_degrees: 45.0,
    };
    assert!(
        crate::world::is_position_walkable_in_space(
            &world,
            world.space_registry(),
            catalogs,
            start,
            agent,
            SpaceId::SURFACE,
        ),
        "surface approach {:?} blocked",
        start.to_global(world.layout())
    );
    assert!(
        crate::world::is_position_walkable_in_space(
            &world,
            world.space_registry(),
            catalogs,
            spawn,
            agent,
            ground_space_id,
        ),
        "interior spawn {:?} blocked",
        spawn.to_global(world.layout())
    );
    assert!(
        crate::world::is_position_walkable_in_space(
            &world,
            world.space_registry(),
            catalogs,
            interior_goal,
            agent,
            ground_space_id,
        ),
        "interior goal {:?} blocked",
        interior_goal.to_global(world.layout())
    );
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
        Some(crate::world::UnitOwnership::player_default()),
    )
    .expect("surface to interior path");
    assert!(path.waypoints.iter().any(|wp| wp.portal_id.is_some()));
    assert!(
        path.waypoints
            .iter()
            .any(|wp| wp.space_id == ground_space_id)
    );
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
    for _ in 0..250 {
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

fn hut_entrance_portal(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> crate::world::PortalId {
    world
        .space_registry()
        .portals()
        .find(|(_, portal)| {
            portal.portal_type == crate::world::PortalType::ExteriorEntrance
                && portal.owning_building_id == Some(building_id)
        })
        .map(|(id, _)| *id)
        .expect("entrance portal")
}

fn hut_interior_spawn(world: &WorldData, building_id: crate::world::BuildingId) -> WorldPosition {
    let portal_id = hut_entrance_portal(world, building_id);
    let portal = world.space_registry().get_portal(portal_id).unwrap();
    portal.to_position
}

fn issue_move_and_resolve(
    world: &mut WorldData,
    unit_catalog: &crate::world::UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    unit_id: crate::world::UnitId,
    target: WorldPosition,
) {
    world
        .command_buffer_mut()
        .enqueue(unit_id, UnitOrder::MoveTo { target });
    let report =
        resolve_pending_unit_orders(world, unit_catalog, catalogs, &NavigationConfig::default());
    assert_eq!(report.resolved, 1, "move order should resolve");
}

fn run_movement_ticks(
    world: &mut WorldData,
    unit_catalog: &crate::world::UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    unit_id: crate::world::UnitId,
    ticks: usize,
    delta_seconds: f32,
) {
    for _ in 0..ticks {
        let _ = step_unit_movement(world, unit_catalog, catalogs, unit_id, delta_seconds);
    }
}

#[test]
fn unit_exits_hut_interior_through_entrance() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();

    let building_id = activate_hut(&mut world);
    let ground_space_id = hut_ground_target(&world, building_id).0;
    let interior_start = hut_interior_spawn(&world, building_id);
    let exterior_goal = hut_entrance_approach(&world, building_id);

    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        interior_start,
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    world
        .set_unit_current_space(unit_id, ground_space_id)
        .expect("set interior space");

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, exterior_goal);

    let layout = world.layout();
    let goal_xz = exterior_goal.to_global(layout).xz();
    let mut exited = false;
    for _ in 0..200 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id.is_surface() {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(goal_xz) < 4.0 {
                exited = true;
                break;
            }
        }
    }
    assert!(exited, "unit should exit hut interior to surface");
}

#[test]
fn unit_round_trips_hut_entrance() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();

    let building_id = activate_hut(&mut world);
    let (ground_space_id, interior_centroid_pos) = hut_ground_target(&world, building_id);
    let portal_id = hut_entrance_portal(&world, building_id);
    let start = hut_entrance_approach(&world, building_id);
    let exterior_goal = {
        let layout = world.layout();
        let portal = world.space_registry().get_portal(portal_id).unwrap();
        let away = portal.from_center_global_xz - start.to_global(layout).xz();
        let away = if away.length_squared() > 1e-4 {
            away.normalize()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let goal_xz = portal.from_center_global_xz + away * 6.0;
        WorldPosition::from_global(Vec3::new(goal_xz.x, 0.0, goal_xz.y), layout)
    };

    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
    )
    .unwrap()
    .id;

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };

    issue_move_and_resolve(
        &mut world,
        &unit_catalog,
        catalogs,
        unit_id,
        interior_centroid_pos,
    );

    let layout = world.layout();
    let interior_goal_xz = interior_centroid_pos.to_global(layout).xz();
    let mut portal_consumed = false;
    let mut entered_interior = false;
    for _ in 0..200 {
        let record_before = world.get_unit(unit_id).unwrap();
        let portal_index = if let UnitState::Moving { ref path, .. } = record_before.state {
            path.waypoints
                .iter()
                .position(|wp| wp.portal_id == Some(portal_id))
        } else {
            None
        };
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if let (UnitState::Moving { waypoint_index, .. }, Some(pi)) = (&record.state, portal_index)
        {
            if *waypoint_index > pi {
                portal_consumed = true;
            }
        }
        if record.current_space_id == ground_space_id {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(interior_goal_xz) < 2.5 {
                entered_interior = true;
                break;
            }
        }
    }
    assert!(entered_interior, "unit should enter hut interior");
    assert!(
        portal_consumed,
        "portal transition waypoint should be consumed"
    );

    let secondary_interior = {
        let runtime = world
            .building_navigation_runtime()
            .get(building_id)
            .unwrap();
        let ground = runtime
            .regions
            .iter()
            .find(|region| region.floor_key == "ground_interior")
            .unwrap();
        let layout = world.layout();
        let inset = 0.7;
        let max = ground.world_aabb_max_xz;
        let xz = max - Vec2::new(inset, inset);
        let floor_y = world
            .space_registry()
            .get_space(ground_space_id)
            .map(|space| space.floor_y_global)
            .unwrap_or(0.0);
        WorldPosition::from_global(Vec3::new(xz.x, floor_y, xz.y), layout)
    };
    issue_move_and_resolve(
        &mut world,
        &unit_catalog,
        catalogs,
        unit_id,
        secondary_interior,
    );
    run_movement_ticks(&mut world, &unit_catalog, catalogs, unit_id, 80, 0.25);

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, exterior_goal);

    let exterior_goal_xz = exterior_goal.to_global(layout).xz();
    let mut returned_surface = false;
    for _ in 0..300 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id.is_surface() {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(exterior_goal_xz) < 6.0 {
                returned_surface = true;
                break;
            }
        }
    }
    assert!(
        returned_surface,
        "unit should exit and reach exterior goal (space={:?} pos={:?})",
        world.get_unit(unit_id).map(|u| u.current_space_id),
        world
            .get_unit(unit_id)
            .map(|u| u.placement.position.to_global(layout).xz()),
    );
}

fn hut_exterior_door_id(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> crate::world::DoorId {
    world
        .door_store()
        .building_door_ids(building_id)
        .iter()
        .find_map(|door_id| {
            world
                .door_store()
                .get(*door_id)
                .filter(|door| door.definition_key == "exterior_entrance")
                .map(|_| *door_id)
        })
        .expect("exterior entrance door")
}

#[test]
fn door_controlled_entrance_round_trip() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();

    let building_id = activate_hut(&mut world);
    let (ground_space_id, interior_goal) = hut_ground_target(&world, building_id);
    let portal_id = hut_entrance_portal(&world, building_id);
    let door_id = hut_exterior_door_id(&world, building_id);
    let start = hut_entrance_approach(&world, building_id);
    let layout = world.layout();

    assert_eq!(
        world.door_store().get(door_id).unwrap().state,
        DoorState::Closed
    );
    assert!(
        !world
            .space_registry()
            .get_portal(portal_id)
            .unwrap()
            .enabled
    );

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
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
        Some(crate::world::UnitOwnership::player_default()),
    )
    .expect("authorized unit may plan through closed openable door");
    assert!(
        path.waypoints
            .iter()
            .any(|wp| wp.portal_id == Some(portal_id))
    );

    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
    )
    .unwrap()
    .id;

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, interior_goal);

    let interior_goal_xz = interior_goal.to_global(layout).xz();
    let mut portal_consumed = false;
    let mut entered_interior = false;
    for _ in 0..250 {
        let record_before = world.get_unit(unit_id).unwrap();
        let portal_index = if let UnitState::Moving { ref path, .. } = record_before.state {
            path.waypoints
                .iter()
                .position(|wp| wp.portal_id == Some(portal_id))
        } else {
            None
        };
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if let (UnitState::Moving { waypoint_index, .. }, Some(pi)) = (&record.state, portal_index)
        {
            if *waypoint_index > pi {
                portal_consumed = true;
            }
        }
        if record.current_space_id == ground_space_id {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(interior_goal_xz) < 2.5 {
                entered_interior = true;
                break;
            }
        }
    }
    assert!(
        entered_interior,
        "unit should enter through door-controlled portal"
    );
    assert!(
        portal_consumed,
        "portal waypoint should be consumed exactly once"
    );
    assert_eq!(
        world.door_store().get(door_id).unwrap().state,
        DoorState::Open
    );
    assert!(
        world
            .space_registry()
            .get_portal(portal_id)
            .unwrap()
            .enabled
    );

    close_door(&mut world, door_id).unwrap();
    let portal_record = world.space_registry().get_portal(portal_id).unwrap();
    assert!(!portal_record.enabled);
    assert!(!portal_record.can_traverse_from(ground_space_id));

    open_door(&mut world, door_id).unwrap();
    let exterior_goal = hut_entrance_approach(&world, building_id);
    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, exterior_goal);

    let goal_xz = exterior_goal.to_global(layout).xz();
    let mut exited = false;
    for _ in 0..250 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id.is_surface() {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(goal_xz) < 4.0 {
                exited = true;
                break;
            }
        }
    }
    assert!(exited, "unit should exit after door reopens");
    assert_eq!(record_space(&world, unit_id), SpaceId::SURFACE);
}

fn hut_upper_target(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> (SpaceId, WorldPosition) {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let upper = runtime
        .regions
        .iter()
        .find(|region| region.floor_key == "upper_interior")
        .expect("upper floor");
    let layout = world.layout();
    let position = agent_clear_interior_position(upper, layout, upper.space_id, world, 0.6);
    (upper.space_id, position)
}

fn hut_stairs_portal(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> crate::world::PortalId {
    world
        .space_registry()
        .portals()
        .find(|(_, portal)| {
            portal.portal_type == crate::world::PortalType::Stair
                && portal.owning_building_id == Some(building_id)
        })
        .map(|(id, _)| *id)
        .expect("stairs portal")
}

fn run_until<F>(
    world: &mut WorldData,
    unit_catalog: &crate::world::UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    unit_id: crate::world::UnitId,
    max_ticks: usize,
    delta_seconds: f32,
    mut predicate: F,
) -> bool
where
    F: FnMut(&WorldData, crate::world::UnitId) -> bool,
{
    for _ in 0..max_ticks {
        if predicate(world, unit_id) {
            return true;
        }
        let _ = step_unit_movement(world, unit_catalog, catalogs, unit_id, delta_seconds);
    }
    predicate(world, unit_id)
}

#[test]
fn overlapping_floor_tracked_upper_stays_at_upper_elevation() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world);
    let (ground_space_id, _) = hut_ground_target(&world, building_id);
    let (upper_space_id, upper_pos) = hut_upper_target(&world, building_id);
    let resolved = resolve_navigation_start_space(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        upper_pos,
        upper_space_id,
    );
    assert_eq!(resolved, upper_space_id);
    assert_ne!(resolved, ground_space_id);
}

#[test]
fn overlapping_floor_goal_resolves_by_elevation() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world);
    let (upper_space_id, upper_pos) = hut_upper_target(&world, building_id);
    let resolved = resolve_navigation_space_at_position(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        upper_pos,
    );
    assert_eq!(resolved, upper_space_id);
}

#[test]
fn unit_traverses_stairs_up_and_down() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();

    let building_id = activate_hut(&mut world);
    let (ground_space_id, ground_goal) = hut_ground_target(&world, building_id);
    let (upper_space_id, upper_goal) = hut_upper_target(&world, building_id);
    let stairs_portal = hut_stairs_portal(&world, building_id);
    let interior_start = hut_interior_spawn(&world, building_id);

    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        interior_start,
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    world
        .set_unit_current_space(unit_id, ground_space_id)
        .expect("set ground space");

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, upper_goal);

    let layout = world.layout();
    let upper_goal_xz = upper_goal.to_global(layout).xz();
    let mut stair_portal_consumed = false;
    let mut reached_upper = false;
    for _ in 0..250 {
        let record = world.get_unit(unit_id).unwrap();
        if let UnitState::Moving {
            ref path,
            waypoint_index,
            ..
        } = record.state
        {
            if let Some(pi) = path
                .waypoints
                .iter()
                .position(|wp| wp.portal_id == Some(stairs_portal))
            {
                if waypoint_index > pi {
                    stair_portal_consumed = true;
                }
            }
        }
        if record.current_space_id == upper_space_id {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(upper_goal_xz) < 2.5 {
                reached_upper = true;
                break;
            }
        }
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
    }
    assert!(reached_upper, "unit should reach upper floor via stairs");
    assert!(
        stair_portal_consumed,
        "stairs portal waypoint should be consumed"
    );
    let upper_y = world
        .space_registry()
        .get_space(upper_space_id)
        .unwrap()
        .floor_y_global;
    assert!(
        (world
            .get_unit(unit_id)
            .unwrap()
            .placement
            .position
            .to_global(layout)
            .y
            - upper_y)
            .abs()
            < 0.25
    );

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, ground_goal);
    let ground_goal_xz = ground_goal.to_global(layout).xz();
    let reached_ground = run_until(
        &mut world,
        &unit_catalog,
        catalogs,
        unit_id,
        250,
        0.25,
        |world, unit_id| {
            let record = world.get_unit(unit_id).unwrap();
            record.current_space_id == ground_space_id
                && record
                    .placement
                    .position
                    .to_global(layout)
                    .xz()
                    .distance(ground_goal_xz)
                    < 2.5
        },
    );
    assert!(
        reached_ground,
        "unit should return to ground floor via stairs"
    );
    let ground_y = world
        .space_registry()
        .get_space(ground_space_id)
        .unwrap()
        .floor_y_global;
    assert!(
        (world
            .get_unit(unit_id)
            .unwrap()
            .placement
            .position
            .to_global(layout)
            .y
            - ground_y)
            .abs()
            < 0.25
    );
}

#[test]
fn unit_full_multi_floor_surface_round_trip() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();

    let building_id = activate_hut(&mut world);
    let (ground_space_id, ground_goal) = hut_ground_target(&world, building_id);
    let (upper_space_id, upper_goal) = hut_upper_target(&world, building_id);
    let entrance_portal = hut_entrance_portal(&world, building_id);
    let stairs_portal = hut_stairs_portal(&world, building_id);
    let start = hut_entrance_approach(&world, building_id);
    let layout = world.layout();
    let exterior_goal = {
        let portal = world.space_registry().get_portal(entrance_portal).unwrap();
        let away = portal.from_center_global_xz - start.to_global(layout).xz();
        let away = if away.length_squared() > 1e-4 {
            away.normalize()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let goal_xz = portal.from_center_global_xz + away * 6.0;
        WorldPosition::from_global(Vec3::new(goal_xz.x, 0.0, goal_xz.y), layout)
    };

    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
    )
    .unwrap()
    .id;

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, ground_goal);
    assert!(run_until(
        &mut world,
        &unit_catalog,
        catalogs,
        unit_id,
        250,
        0.25,
        |world, unit_id| world.get_unit(unit_id).unwrap().current_space_id == ground_space_id,
    ));

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, upper_goal);
    assert!(run_until(
        &mut world,
        &unit_catalog,
        catalogs,
        unit_id,
        300,
        0.25,
        |world, unit_id| world.get_unit(unit_id).unwrap().current_space_id == upper_space_id,
    ));

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, ground_goal);
    assert!(run_until(
        &mut world,
        &unit_catalog,
        catalogs,
        unit_id,
        300,
        0.25,
        |world, unit_id| world.get_unit(unit_id).unwrap().current_space_id == ground_space_id,
    ));

    issue_move_and_resolve(&mut world, &unit_catalog, catalogs, unit_id, exterior_goal);

    let exterior_goal_xz = exterior_goal.to_global(layout).xz();
    let mut returned_surface = false;
    for _ in 0..400 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id.is_surface() {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(exterior_goal_xz) < 6.0 {
                returned_surface = true;
                break;
            }
        }
    }
    assert!(
        returned_surface,
        "unit should exit and reach exterior goal (space={:?} pos={:?})",
        world.get_unit(unit_id).map(|u| u.current_space_id),
        world
            .get_unit(unit_id)
            .map(|u| u.placement.position.to_global(layout).xz()),
    );
    let record = world.get_unit(unit_id).unwrap();
    assert_eq!(record.current_space_id, SpaceId::SURFACE);
    let _ = (entrance_portal, stairs_portal);
}

fn record_space(world: &WorldData, unit_id: crate::world::UnitId) -> SpaceId {
    world.get_unit(unit_id).unwrap().current_space_id
}
