//! NAV-ENTRY movement-tick regressions for occluded Surface→Interior approach.

use bevy::prelude::*;

use super::fixtures::one_region_doorless_navigation_blueprint;
use super::surface_entry_diagnostics::{
    diagnose_surface_to_interior_ingress, format_ingress_diagnostic,
};
use super::surface_exit_tests::{
    ROBOT_RADIUS, activate_fixture, default_catalogs, entrance_portal_for_building,
    local_xz_to_world, pass_catalogs, pos, region_space,
};
use super::surface_support::resolve_surface_entrance_approach_position;
use crate::world::unit::{
    UnitMovementStepOutcome, UnitOrder, UnitSource, UnitState, create_unit, step_unit_movement,
};
use crate::world::{
    NavigationConfig, SpaceId, UnitCatalog, UnitDefinition, UnitDefinitionId, UnitRenderKey,
    WeaponDefinitionId, WorldData, resolve_pending_unit_orders,
};

const MAX_SLOPE: f32 = 45.0;
const MAX_TICKS: usize = 900;
const TICK_SECONDS: f32 = 0.25;
const BLOCKED_AT_SUPPORT_LIMIT: u32 = 30;

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
enum EntryMovementPhase {
    OnSurfaceApproaching,
    EnteredInterior,
    ArrivedInteriorGoal,
}

#[derive(Debug)]
struct EntryMovementReport {
    phase: EntryMovementPhase,
    portal_transitions: u32,
    consecutive_blocked_on_surface: u32,
    max_consecutive_blocked_on_surface: u32,
}

impl EntryMovementReport {
    fn assert_enters_interior_around_building(&self) {
        assert!(
            matches!(
                self.phase,
                EntryMovementPhase::EnteredInterior | EntryMovementPhase::ArrivedInteriorGoal
            ),
            "unit never entered interior (phase={:?}, portal_transitions={})",
            self.phase,
            self.portal_transitions
        );
        assert!(
            self.max_consecutive_blocked_on_surface <= BLOCKED_AT_SUPPORT_LIMIT,
            "unit stalled against support ({} consecutive blocked surface ticks)",
            self.max_consecutive_blocked_on_surface
        );
        assert!(
            self.portal_transitions >= 1,
            "expected at least one portal transition"
        );
    }
}

fn execute_surface_to_interior_movement(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    building_id: crate::world::BuildingId,
    surface_start: crate::world::WorldPosition,
    interior_goal: crate::world::WorldPosition,
    interior_space: SpaceId,
) -> EntryMovementReport {
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let nav_config = NavigationConfig::default();
    let layout = world.layout();

    let unit_id = create_unit(
        unit_catalog,
        world,
        &UnitDefinitionId::new("robot"),
        surface_start,
        UnitSource::Authored,
    )
    .expect("spawn robot")
    .id;

    world.command_buffer_mut().enqueue(
        unit_id,
        UnitOrder::MoveTo {
            target: interior_goal,
        },
    );

    let portal = entrance_portal_for_building(world, building_id);
    let resolve_result = resolve_pending_unit_orders(world, unit_catalog, catalogs, &nav_config);
    if resolve_result.resolved == 0 {
        let diagnostic = diagnose_surface_to_interior_ingress(
            world,
            catalogs,
            &nav_config,
            ROBOT_RADIUS,
            surface_start,
            interior_goal,
            SpaceId::SURFACE,
            interior_space,
            &portal,
        );
        panic!(
            "move order must resolve (resolved={})\n{}",
            resolve_result.resolved,
            format_ingress_diagnostic(&diagnostic)
        );
    }
    assert_eq!(resolve_result.resolved, 1, "move order must resolve");

    let approach = resolve_surface_entrance_approach_position(
        world,
        world.space_registry(),
        &portal,
        ROBOT_RADIUS,
    )
    .expect("approach position");
    let approach_xz = approach.to_global(layout).xz();
    let goal_xz = interior_goal.to_global(layout).xz();

    let mut report = EntryMovementReport {
        phase: EntryMovementPhase::OnSurfaceApproaching,
        portal_transitions: 0,
        consecutive_blocked_on_surface: 0,
        max_consecutive_blocked_on_surface: 0,
    };

    for _ in 0..MAX_TICKS {
        let record = world.get_unit(unit_id).expect("unit");
        if record.current_space_id == interior_space {
            report.phase = EntryMovementPhase::EnteredInterior;
            report.portal_transitions = report.portal_transitions.max(1);
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(goal_xz) < 1.0 {
                report.phase = EntryMovementPhase::ArrivedInteriorGoal;
                break;
            }
        } else if record.current_space_id.is_surface() {
            let pos_xz = record.placement.position.to_global(layout).xz();
            if pos_xz.distance(approach_xz) < 2.0 {
                report.phase = EntryMovementPhase::OnSurfaceApproaching;
            }
        }

        if matches!(record.state, UnitState::Idle) && record.current_space_id == interior_space {
            report.phase = EntryMovementPhase::ArrivedInteriorGoal;
            break;
        }

        let on_surface = record.current_space_id.is_surface();
        let outcome = step_unit_movement(world, unit_catalog, catalogs, unit_id, TICK_SECONDS);
        if on_surface && matches!(outcome, UnitMovementStepOutcome::Blocked(_)) {
            report.consecutive_blocked_on_surface += 1;
            report.max_consecutive_blocked_on_surface = report
                .max_consecutive_blocked_on_surface
                .max(report.consecutive_blocked_on_surface);
        } else if on_surface {
            report.consecutive_blocked_on_surface = 0;
        }

        if world.get_unit(unit_id).unwrap().current_space_id == interior_space {
            report.portal_transitions = report.portal_transitions.max(1);
        }
    }

    report
}

#[test]
fn occluded_entrance_movement_routes_around_building_and_enters() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let unit_catalog = robot_catalog();
    let report = execute_surface_to_interior_movement(
        &mut world,
        &unit_catalog,
        building_id,
        start,
        goal,
        interior,
    );
    report.assert_enters_interior_around_building();
}

#[test]
fn entrance_facing_movement_enters_interior_without_stalling() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let interior = region_space(&world, building_id, "ground", "main");
    let portal = entrance_portal_for_building(&world, building_id);
    let layout = world.layout();
    let start = {
        let approach = portal.from_center_global_xz + Vec2::new(3.0, 0.0);
        crate::world::WorldPosition::from_global(Vec3::new(approach.x, 0.0, approach.y), layout)
    };
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let unit_catalog = robot_catalog();
    let report = execute_surface_to_interior_movement(
        &mut world,
        &unit_catalog,
        building_id,
        start,
        goal,
        interior,
    );
    report.assert_enters_interior_around_building();
}
