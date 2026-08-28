//! NAV-WAYPOINT: monotonic heading lookahead / persistent waypoint progression regressions.

use super::id::UnitId;
use super::state::UnitState;
use crate::world::movement::feel::{heading_lookahead_commit_index, stabilized_movement_heading};
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, Heightfield, NavigationPath,
    PortalId, SpaceId, TestPassabilityBundle, UnitCatalog, UnitDefinition, UnitDefinitionId,
    UnitRenderKey, UnitSource, WeaponDefinitionId, WorldData, WorldPosition, create_unit,
    step_unit_movement, xz_distance,
};

const ROBOT_RADIUS: f32 = 0.6;
const MAX_SLOPE: f32 = 45.0;
const TICK_SECONDS: f32 = 0.25;

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn flat_chunk_dense(height: f32) -> ChunkData {
    let edge: u32 = 65;
    let count = edge as usize * edge as usize;
    let heightfield = Heightfield::from_samples(edge, 4.0, vec![height; count]).unwrap();
    ChunkData::new(heightfield, Vec::new())
}

fn insert_flat_dense(world: &mut WorldData, x: i32, z: i32, height: f32) {
    let chunk = ChunkId::new(ChunkCoord::new(x, z));
    world.insert(chunk, flat_chunk_dense(height));
}

fn pos_global(x: f32, z: f32) -> WorldPosition {
    WorldPosition::from_global(bevy::prelude::Vec3::new(x, 0.0, z), layout())
}

fn robot_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(vec![UnitDefinition::new_test(
        UnitDefinitionId::new("robot"),
        "Robot",
        "Player",
        1,
        100,
        100,
        5,
        5,
        5,
        5,
        5,
        5,
        10.0,
        "Common",
        9.0,
        ROBOT_RADIUS,
        MAX_SLOPE,
        WeaponDefinitionId::new("weapon_fists"),
        true,
        UnitRenderKey::reserved("robot"),
    )])
    .expect("robot catalog")
}

fn step(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    doodad: &DoodadCatalog,
    unit_id: UnitId,
) -> super::movement::UnitMovementStepOutcome {
    let bundle = TestPassabilityBundle::new();
    step_unit_movement(
        world,
        catalog,
        bundle.catalogs_for(doodad),
        unit_id,
        TICK_SECONDS,
    )
}

fn spawn_moving_unit(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    position: WorldPosition,
    path: NavigationPath,
    waypoint_index: usize,
    target: WorldPosition,
) -> UnitId {
    let unit_id = create_unit(
        catalog,
        world,
        &UnitDefinitionId::new("robot"),
        position,
        UnitSource::Authored,
    )
    .expect("spawn")
    .id;
    world
        .set_unit_state(
            unit_id,
            UnitState::Moving {
                target,
                path,
                waypoint_index,
            },
        )
        .expect("moving state");
    unit_id
}

fn effective_index(record: &crate::world::UnitRecord, layout: ChunkLayout) -> usize {
    let UnitState::Moving {
        path,
        waypoint_index,
        ..
    } = &record.state
    else {
        panic!("expected moving");
    };
    stabilized_movement_heading(record.placement.position, path, *waypoint_index, layout)
        .map(|heading| heading.waypoint_index)
        .unwrap_or(*waypoint_index)
}

fn state_waypoint_index(record: &crate::world::UnitRecord) -> usize {
    match &record.state {
        UnitState::Moving { waypoint_index, .. } => *waypoint_index,
        _ => panic!("expected moving"),
    }
}

#[test]
fn heading_reverts_without_commit_when_distance_grows_past_lookahead() {
    let escape = pos_global(866.12, 1043.47);
    let next = pos_global(868.21, 1063.48);
    let path = NavigationPath::from_surface_positions(vec![escape, next]);
    let unit_pos = pos_global(866.20, 1043.55);
    assert!(
        xz_distance(unit_pos, escape, layout()) > 0.05
            && xz_distance(unit_pos, escape, layout()) < 0.25
    );
    let heading = stabilized_movement_heading(unit_pos, &path, 0, layout()).expect("heading");
    assert_eq!(heading.waypoint_index, 1, "lookahead selects next waypoint");
    let moved = pos_global(866.35, 1043.70);
    assert!(xz_distance(moved, escape, layout()) > 0.25);
    let regressed = stabilized_movement_heading(moved, &path, 0, layout()).expect("heading");
    assert_eq!(
        regressed.waypoint_index, 0,
        "pre-commit stale state regresses to earlier waypoint"
    );
}

#[test]
fn straight_escape_geometry_commits_and_does_not_regress() {
    let escape = pos_global(866.12, 1043.47);
    let next = pos_global(868.21, 1063.48);
    let path = NavigationPath::from_surface_positions(vec![escape, next]);
    let mut world = WorldData::new(layout());
    insert_flat_dense(&mut world, 3, 4, 0.0);
    let doodad = DoodadCatalog::default();
    let unit_catalog = robot_catalog();
    let unit_id = spawn_moving_unit(
        &mut world,
        &unit_catalog,
        pos_global(866.20, 1043.55),
        path,
        0,
        next,
    );
    step(&mut world, &unit_catalog, &doodad, unit_id);
    let record = world.get_unit(unit_id).expect("unit");
    assert_eq!(
        state_waypoint_index(record),
        1,
        "lookahead must commit persistent index past escape"
    );
    assert_eq!(effective_index(record, layout()), 1);
    world
        .update_unit_position(unit_id, pos_global(866.35, 1043.70))
        .expect("relocate");
    step(&mut world, &unit_catalog, &doodad, unit_id);
    let record = world.get_unit(unit_id).expect("unit");
    assert!(
        state_waypoint_index(record) >= 1,
        "persistent index must not regress to escape"
    );
    assert!(
        effective_index(record, layout()) >= 1,
        "effective index must not regress after commit"
    );
}

#[test]
fn back_side_escape_geometry_commits_and_does_not_regress() {
    let escape = pos_global(866.12, 1043.47);
    let next = pos_global(854.00, 1038.00);
    let path = NavigationPath::from_surface_positions(vec![escape, next]);
    let mut world = WorldData::new(layout());
    insert_flat_dense(&mut world, 3, 4, 0.0);
    let doodad = DoodadCatalog::default();
    let unit_catalog = robot_catalog();
    let unit_id = spawn_moving_unit(
        &mut world,
        &unit_catalog,
        pos_global(866.18, 1043.52),
        path,
        0,
        next,
    );
    step(&mut world, &unit_catalog, &doodad, unit_id);
    let record = world.get_unit(unit_id).expect("unit");
    assert_eq!(state_waypoint_index(record), 1);
    world
        .update_unit_position(unit_id, pos_global(865.95, 1043.35))
        .expect("relocate");
    step(&mut world, &unit_catalog, &doodad, unit_id);
    let record = world.get_unit(unit_id).expect("unit");
    assert!(state_waypoint_index(record) >= 1);
    assert!(effective_index(record, layout()) >= 1);
}

#[test]
fn diagonal_approach_still_progresses_past_committed_waypoint() {
    let escape = pos_global(866.12, 1043.47);
    let next = pos_global(865.0, 1045.0);
    let path = NavigationPath::from_surface_positions(vec![escape, next]);
    let mut world = WorldData::new(layout());
    insert_flat_dense(&mut world, 3, 4, 0.0);
    let doodad = DoodadCatalog::default();
    let unit_catalog = robot_catalog();
    let unit_id = spawn_moving_unit(
        &mut world,
        &unit_catalog,
        pos_global(866.05, 1043.40),
        path,
        0,
        next,
    );
    for _ in 0..40 {
        step(&mut world, &unit_catalog, &doodad, unit_id);
        let record = world.get_unit(unit_id).expect("unit");
        if matches!(record.state, UnitState::Idle) {
            break;
        }
        assert!(
            state_waypoint_index(record) > 0
                || xz_distance(record.placement.position, escape, layout()) <= 0.05,
            "must not oscillate back to escape after commit"
        );
    }
}

#[test]
fn portal_waypoint_is_not_committed_by_lookahead() {
    use crate::world::NavigationWaypoint;
    let portal = PortalId::new(9);
    let portal_pos = pos_global(10.0, 10.0);
    let interior = SpaceId::new(3);
    let path = NavigationPath::new(vec![
        NavigationWaypoint::in_space(pos_global(9.9, 9.9), SpaceId::SURFACE),
        NavigationWaypoint::portal_transition(portal_pos, SpaceId::SURFACE, portal),
        NavigationWaypoint::in_space(pos_global(11.0, 11.0), interior),
    ]);
    assert_eq!(heading_lookahead_commit_index(&path, 0, 1), None);
    let mut world = WorldData::new(layout());
    insert_flat_dense(&mut world, 0, 0, 0.0);
    let doodad = DoodadCatalog::default();
    let unit_catalog = robot_catalog();
    let unit_id = spawn_moving_unit(
        &mut world,
        &unit_catalog,
        pos_global(9.92, 9.92),
        path,
        1,
        pos_global(11.0, 11.0),
    );
    step(&mut world, &unit_catalog, &doodad, unit_id);
    let record = world.get_unit(unit_id).expect("unit");
    assert_eq!(
        state_waypoint_index(record),
        1,
        "portal waypoint must remain authoritative until portal completion"
    );
}

#[test]
fn ordinary_multi_waypoint_path_reaches_destination() {
    let mut world = WorldData::new(layout());
    insert_flat_dense(&mut world, 0, 0, 0.0);
    let doodad = DoodadCatalog::default();
    let unit_catalog = robot_catalog();
    let goal = pos_global(40.0, 30.0);
    let path = NavigationPath::from_surface_positions(vec![
        pos_global(10.0, 0.0),
        pos_global(20.0, 10.0),
        pos_global(30.0, 20.0),
        goal,
    ]);
    let unit_id = spawn_moving_unit(
        &mut world,
        &unit_catalog,
        pos_global(0.0, 0.0),
        path,
        0,
        goal,
    );
    for _ in 0..200 {
        step(&mut world, &unit_catalog, &doodad, unit_id);
        if matches!(
            world.get_unit(unit_id).expect("unit").state,
            UnitState::Idle
        ) {
            break;
        }
    }
    assert!(matches!(
        world.get_unit(unit_id).expect("unit").state,
        UnitState::Idle
    ));
}

#[test]
fn sharp_turn_path_does_not_overrun_waypoints() {
    let mut world = WorldData::new(layout());
    insert_flat_dense(&mut world, 0, 0, 0.0);
    let doodad = DoodadCatalog::default();
    let unit_catalog = robot_catalog();
    let goal = pos_global(0.0, 40.0);
    let path = NavigationPath::from_surface_positions(vec![
        pos_global(0.0, 10.0),
        pos_global(20.0, 10.0),
        pos_global(20.0, 30.0),
        goal,
    ]);
    let unit_id = spawn_moving_unit(
        &mut world,
        &unit_catalog,
        pos_global(0.0, 0.0),
        path,
        0,
        goal,
    );
    let mut max_index = 0usize;
    for _ in 0..300 {
        step(&mut world, &unit_catalog, &doodad, unit_id);
        let record = world.get_unit(unit_id).expect("unit");
        if let UnitState::Moving {
            waypoint_index,
            path,
            ..
        } = &record.state
        {
            assert!(*waypoint_index < path.len());
            max_index = max_index.max(*waypoint_index);
        } else {
            break;
        }
    }
    assert!(max_index <= 3);
    assert!(matches!(
        world.get_unit(unit_id).expect("unit").state,
        UnitState::Idle
    ));
}
