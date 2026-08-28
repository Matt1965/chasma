//! NAV-EXIT-R1: movement-tick regressions through Interior→Surface escape seam.

use bevy::prelude::*;

use super::surface_exit_tests::{
    ROBOT_RADIUS, activate_fixture, default_catalogs, entrance_portal_for_building,
    exit_hut_blueprint, local_xz_to_world, pass_catalogs, region_space, surface_local_xz_to_world,
};
use super::surface_support::resolve_surface_entrance_escape_position;
use crate::world::unit::{UnitMovementStepOutcome, UnitOrder, UnitSource, UnitState, create_unit};
use crate::world::{
    DoodadCatalog, FootprintCatalog, NavigationConfig, SpaceId, UnitCatalog, UnitDefinition,
    UnitDefinitionId, UnitOwnership, UnitRenderKey, WeaponDefinitionId, WorldData,
    find_path_with_spaces, resolve_pending_unit_orders, step_unit_movement, xz_distance,
};

const MAX_SLOPE: f32 = 45.0;
const MAX_TICKS: usize = 800;
const TICK_SECONDS: f32 = 0.25;
const SEAM_BLOCKED_TICK_LIMIT: u32 = 20;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitMovementPhase {
    BeforeSurface,
    AtOrBeforeEscape,
    PastEscapeSeam,
    Arrived,
}

#[derive(Debug)]
struct ExitMovementReport {
    phase: ExitMovementPhase,
    escape_index: Option<usize>,
    max_waypoint_index: usize,
    blocked_ticks_at_escape_index: u32,
    consecutive_blocked_at_escape: u32,
    max_consecutive_blocked_at_escape: u32,
    portal_transitions: u32,
}

impl ExitMovementReport {
    fn assert_progresses_past_escape_seam(&self, label: &str) {
        assert!(
            matches!(
                self.phase,
                ExitMovementPhase::PastEscapeSeam | ExitMovementPhase::Arrived
            ),
            "{label}: unit never progressed past escape seam (phase={:?}, max_index={}, escape_index={:?})",
            self.phase,
            self.max_waypoint_index,
            self.escape_index
        );
        assert!(
            self.max_consecutive_blocked_at_escape <= SEAM_BLOCKED_TICK_LIMIT,
            "{label}: seam jitter detected ({} consecutive blocked ticks at escape index)",
            self.max_consecutive_blocked_at_escape
        );
    }
}

fn execute_interior_to_surface_movement(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    building_id: crate::world::BuildingId,
    interior_start: crate::world::WorldPosition,
    surface_goal: crate::world::WorldPosition,
    interior_space: SpaceId,
) -> ExitMovementReport {
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let nav_config = NavigationConfig::default();
    let layout = world.layout();

    let unit_id = create_unit(
        unit_catalog,
        world,
        &UnitDefinitionId::new("robot"),
        interior_start,
        UnitSource::Authored,
    )
    .expect("spawn robot")
    .id;
    world
        .set_unit_current_space(unit_id, interior_space)
        .expect("interior membership");

    world.command_buffer_mut().enqueue(
        unit_id,
        UnitOrder::MoveTo {
            target: surface_goal,
        },
    );
    assert_eq!(
        resolve_pending_unit_orders(world, unit_catalog, catalogs, &nav_config).resolved,
        1,
        "move order must resolve"
    );

    let portal = entrance_portal_for_building(world, building_id);
    let escape = resolve_surface_entrance_escape_position(
        world,
        world.space_registry(),
        &portal,
        ROBOT_RADIUS,
    )
    .expect("escape position");
    let escape_xz = escape.to_global(layout).xz();
    let escape_index = world
        .get_unit(unit_id)
        .and_then(|record| match &record.state {
            UnitState::Moving { path, .. } => path.waypoints.iter().position(|waypoint| {
                waypoint.space_id == SpaceId::SURFACE
                    && waypoint.portal_id.is_none()
                    && waypoint.position.to_global(layout).xz().distance(escape_xz) < 1.0
            }),
            _ => None,
        });

    let goal_xz = surface_goal.to_global(layout).xz();
    let mut report = ExitMovementReport {
        phase: ExitMovementPhase::BeforeSurface,
        escape_index,
        max_waypoint_index: 0,
        blocked_ticks_at_escape_index: 0,
        consecutive_blocked_at_escape: 0,
        max_consecutive_blocked_at_escape: 0,
        portal_transitions: 0,
    };

    for _ in 0..MAX_TICKS {
        let outcome = step_unit_movement(world, unit_catalog, catalogs, unit_id, TICK_SECONDS);
        let record = world.get_unit(unit_id).expect("unit");
        let pos_xz = record.placement.position.to_global(layout).xz();

        if matches!(record.state, UnitState::Idle) && pos_xz.distance(goal_xz) < 2.5 {
            report.phase = ExitMovementPhase::Arrived;
            break;
        }

        let UnitState::Moving {
            path,
            waypoint_index,
            ..
        } = &record.state
        else {
            continue;
        };

        report.max_waypoint_index = report.max_waypoint_index.max(*waypoint_index);

        if record.current_space_id.is_surface() {
            if let Some(escape_idx) = escape_index {
                if *waypoint_index > escape_idx {
                    report.phase = ExitMovementPhase::PastEscapeSeam;
                } else if *waypoint_index == escape_idx {
                    report.phase = ExitMovementPhase::AtOrBeforeEscape;
                    if matches!(outcome, UnitMovementStepOutcome::Blocked(_)) {
                        report.blocked_ticks_at_escape_index += 1;
                        report.consecutive_blocked_at_escape += 1;
                        report.max_consecutive_blocked_at_escape = report
                            .max_consecutive_blocked_at_escape
                            .max(report.consecutive_blocked_at_escape);
                    } else {
                        report.consecutive_blocked_at_escape = 0;
                    }
                }
            }
        }

        if path
            .waypoints
            .iter()
            .take(*waypoint_index + 1)
            .any(|wp| wp.portal_id.is_some())
            && record.current_space_id.is_surface()
        {
            report.portal_transitions = report.portal_transitions.max(1);
        }
    }

    report
}

#[cfg(feature = "dev")]
#[test]
fn post_exit_trace_arms_after_exterior_entrance_transition() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        exit_hut_blueprint(),
        super::surface_exit_tests::pos(80.0, 80.0),
    );
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(4.0, -6.0));
    let unit_catalog = robot_catalog();
    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("robot"),
        start,
        UnitSource::Authored,
    )
    .expect("spawn")
    .id;
    world
        .set_unit_current_space(unit_id, interior)
        .expect("interior membership");
    world
        .command_buffer_mut()
        .enqueue(unit_id, UnitOrder::MoveTo { target: goal });
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    assert_eq!(
        resolve_pending_unit_orders(
            &mut world,
            &unit_catalog,
            catalogs,
            &NavigationConfig::default()
        )
        .resolved,
        1,
        "resolve move"
    );
    let mut armed = false;
    for _ in 0..MAX_TICKS {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, TICK_SECONDS);
        if world.post_exit_jitter_trace().is_active_for(unit_id) {
            armed = true;
            break;
        }
        if world
            .get_unit(unit_id)
            .is_some_and(|record| record.current_space_id.is_surface())
        {
            break;
        }
    }
    assert!(
        armed,
        "post-exit trace must arm after Interior→Surface portal completion"
    );
}

#[test]
fn straight_out_movement_progresses_past_escape_seam() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        exit_hut_blueprint(),
        super::surface_exit_tests::pos(80.0, 80.0),
    );
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(4.0, -6.0));
    let unit_catalog = robot_catalog();
    let report = execute_interior_to_surface_movement(
        &mut world,
        &unit_catalog,
        building_id,
        start,
        goal,
        interior,
    );
    assert!(
        report.portal_transitions >= 1,
        "must traverse entrance portal"
    );
    report.assert_progresses_past_escape_seam("straight-out");
}

#[test]
fn diagonal_out_movement_control_still_succeeds() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        exit_hut_blueprint(),
        super::surface_exit_tests::pos(80.0, 80.0),
    );
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(-1.0, -5.0));
    let unit_catalog = robot_catalog();
    let report = execute_interior_to_surface_movement(
        &mut world,
        &unit_catalog,
        building_id,
        start,
        goal,
        interior,
    );
    report.assert_progresses_past_escape_seam("diagonal control");
}

#[test]
fn back_side_movement_progresses_past_escape_seam() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        exit_hut_blueprint(),
        super::surface_exit_tests::pos(80.0, 80.0),
    );
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(-2.0, 8.0));
    let unit_catalog = robot_catalog();
    let report = execute_interior_to_surface_movement(
        &mut world,
        &unit_catalog,
        building_id,
        start,
        goal,
        interior,
    );
    report.assert_progresses_past_escape_seam("back-side");
}

#[test]
fn stitched_path_escape_seam_has_single_effective_position() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        exit_hut_blueprint(),
        super::surface_exit_tests::pos(80.0, 80.0),
    );
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(4.0, -6.0));
    let (doodad, building, footprint) = default_catalogs();
    let path = find_path_with_spaces(
        &world,
        pass_catalogs(&doodad, &building, &footprint),
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        interior,
        SpaceId::SURFACE,
        None,
    )
    .expect("straight-out path");
    let portal = entrance_portal_for_building(&world, building_id);
    let escape = resolve_surface_entrance_escape_position(
        &world,
        world.space_registry(),
        &portal,
        ROBOT_RADIUS,
    )
    .expect("escape");
    let layout = world.layout();
    let mut escape_indices = Vec::new();
    for (index, waypoint) in path.waypoints.iter().enumerate() {
        if waypoint.space_id == SpaceId::SURFACE
            && waypoint.portal_id.is_none()
            && xz_distance(waypoint.position, escape, layout) <= 0.05
        {
            escape_indices.push(index);
        }
    }
    assert_eq!(
        escape_indices.len(),
        1,
        "stitched path must contain one effective escape waypoint, got indices {escape_indices:?}"
    );
}
