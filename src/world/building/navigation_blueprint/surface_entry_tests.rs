//! NAV-ENTRY regressions: obstacle-aware Surface→Interior ExteriorEntrance approach.

use bevy::prelude::*;

use super::fixtures::one_region_doorless_navigation_blueprint;
use super::surface_entry_diagnostics::{
    IngressFailureStage, diagnose_surface_to_interior_ingress, format_ingress_diagnostic,
};
use super::surface_exit_tests::{
    ROBOT_RADIUS, activate_fixture, default_catalogs, entrance_portal_for_building,
    local_xz_to_world, pass_catalogs, pos, region_space,
};
use super::surface_support::resolve_surface_entrance_approach_position;
use crate::world::{
    NavigationAgent, NavigationConfig, NavigationPath, PassabilityAgent, PassabilityCatalogs,
    PassabilityResult, SpaceId, WorldData, WorldPosition, find_path_with_spaces,
    query_navigation_point_legality, query_navigation_segment_legality,
};

const MAX_SLOPE: f32 = 45.0;

fn agent(radius: f32) -> PassabilityAgent {
    PassabilityAgent {
        radius_meters: radius,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn nav_agent(radius: f32) -> NavigationAgent {
    NavigationAgent {
        radius_meters: radius,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn surface_waypoints_before_portal(
    path: &NavigationPath,
) -> Vec<(usize, &crate::world::NavigationWaypoint)> {
    path.waypoints
        .iter()
        .enumerate()
        .take_while(|(_, waypoint)| waypoint.portal_id.is_none())
        .filter(|(_, waypoint)| waypoint.space_id == SpaceId::SURFACE)
        .collect()
}

fn assert_surface_path_legal(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    path: &NavigationPath,
    agent_radius: f32,
) {
    let layout = world.layout();
    let nav_config = NavigationConfig::default();
    let pass_agent = agent(agent_radius);
    let surface = surface_waypoints_before_portal(path);
    assert!(
        !surface.is_empty(),
        "expected Surface waypoints before portal transition"
    );
    for (_, waypoint) in &surface {
        assert!(
            matches!(
                query_navigation_point_legality(
                    world,
                    catalogs,
                    waypoint.position,
                    pass_agent,
                    SpaceId::SURFACE,
                ),
                PassabilityResult::Passable { .. }
            ),
            "surface waypoint {:?} must be point-legal",
            waypoint.position.to_global(layout)
        );
    }
    for window in surface.windows(2) {
        let (_, from) = window[0];
        let (_, to) = window[1];
        assert!(
            query_navigation_segment_legality(
                world,
                world.space_registry(),
                catalogs,
                nav_config,
                SpaceId::SURFACE,
                nav_agent(agent_radius),
                from.position,
                to.position,
                layout,
            )
            .is_legal(),
            "surface segment {:?} -> {:?} must be segment-legal",
            from.position.to_global(layout),
            to.position.to_global(layout)
        );
    }
}

fn assert_no_support_crossing_outside_corridor(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    path: &NavigationPath,
) {
    let layout = world.layout();
    let under = local_xz_to_world(world, building_id, Vec2::new(4.0, 3.0));
    let under_xz = under.to_global(layout).xz();
    for (_, waypoint) in surface_waypoints_before_portal(path) {
        let wp_xz = waypoint.position.to_global(layout).xz();
        if wp_xz.distance(under_xz) < 0.5 {
            panic!(
                "surface waypoint {:?} crosses blueprint support outside corridor",
                wp_xz
            );
        }
    }
}

fn exterior_approach(world: &WorldData, building_id: crate::world::BuildingId) -> WorldPosition {
    let layout = world.layout();
    let portal = entrance_portal_for_building(world, building_id);
    let approach = portal.from_center_global_xz + Vec2::new(3.0, 0.0);
    WorldPosition::from_global(Vec3::new(approach.x, 0.0, approach.y), layout)
}

#[test]
fn occluded_entrance_path_routes_around_support_to_interior() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let portal = entrance_portal_for_building(&world, building_id);
    let layout = world.layout();
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let nav_config = NavigationConfig::default();

    let diagnostic = diagnose_surface_to_interior_ingress(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        &portal,
    );
    assert!(
        diagnostic.approach.resolved,
        "terrain-side approach must resolve\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
    assert!(
        matches!(
            diagnostic.approach.point_legality,
            Some(PassabilityResult::Passable { .. })
        ),
        "terrain-side approach must be point-legal\n{}",
        format_ingress_diagnostic(&diagnostic)
    );

    let path = find_path_with_spaces(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        None,
    )
    .unwrap_or_else(|error| {
        panic!(
            "occluded cross-space path failed: {error:?}\nfirst_stage={}\nfirst_error={:?}\n{}",
            diagnostic.first_failure_stage.label(),
            diagnostic.first_navigation_error,
            format_ingress_diagnostic(&diagnostic)
        );
    });

    let portal_count = path
        .waypoints
        .iter()
        .filter(|waypoint| waypoint.portal_id.is_some())
        .count();
    assert_eq!(portal_count, 1, "expected exactly one portal transition");

    assert_surface_path_legal(&world, catalogs, &path, ROBOT_RADIUS);
    assert_no_support_crossing_outside_corridor(&world, building_id, &path);

    let approach_xz = diagnostic
        .approach
        .position
        .expect("approach")
        .to_global(layout)
        .xz();
    assert!(
        surface_waypoints_before_portal(&path)
            .iter()
            .any(|(_, waypoint)| {
                waypoint
                    .position
                    .to_global(layout)
                    .xz()
                    .distance(approach_xz)
                    < 1.0
            }),
        "path must reach terrain-side approach position"
    );

    let last = path.waypoints.last().unwrap();
    assert_eq!(last.space_id, goal_space);
    let goal_xz = goal.to_global(layout).xz();
    let last_xz = last.position.to_global(layout).xz();
    assert!(last_xz.distance(goal_xz) < 0.25);
}

#[test]
fn occluded_entrance_diagnostic_identifies_first_failure_stage() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let portal = entrance_portal_for_building(&world, building_id);
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let nav_config = NavigationConfig::default();

    let diagnostic = diagnose_surface_to_interior_ingress(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        &portal,
    );

    assert!(
        diagnostic.approach.resolved,
        "approach must resolve for occluded fixture\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
    assert_eq!(
        diagnostic.first_failure_stage,
        IngressFailureStage::None,
        "all ingress stages must succeed with traversable entrance\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
    assert!(
        diagnostic.full_path.result.is_ok(),
        "occluded ingress must succeed once entrance portal is traversable\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
    let leg1 = diagnostic.leg1.as_ref().expect("leg1 probe");
    assert!(
        leg1.astar_result.is_ok(),
        "surface start→approach must path\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
    let leg2 = diagnostic.leg2.as_ref().expect("leg2 probe");
    assert!(
        leg2.astar_result.is_ok(),
        "approach→portal must path\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
    eprintln!(
        "NAV-ENTRY-D1 occluded diagnostic (traversable entrance):\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
}

#[test]
fn doorless_entrance_stays_traversable_with_default_profile() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let portal = entrance_portal_for_building(&world, building_id);
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let nav_config = NavigationConfig::default();

    assert!(
        portal.enabled,
        "doorless entrance must remain enabled with default dev profile present"
    );
    assert!(
        world.door_store().door_for_portal_id(portal.id).is_none(),
        "profile door must not bind to doorless entrance"
    );

    let diagnostic = diagnose_surface_to_interior_ingress(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        &portal,
    );

    assert!(
        diagnostic.approach.resolved,
        "terrain-side approach must resolve for doorless entrance\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
    assert_eq!(
        diagnostic.first_failure_stage,
        IngressFailureStage::None,
        "{}",
        format_ingress_diagnostic(&diagnostic)
    );
    assert!(
        diagnostic.full_path.result.is_ok(),
        "doorless entrance must plan full ingress path\n{}",
        format_ingress_diagnostic(&diagnostic)
    );
}

#[test]
fn entrance_facing_surface_to_interior_stays_direct_when_legal() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = exterior_approach(&world, building_id);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);

    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        None,
    )
    .expect("entrance-facing path");

    assert_surface_path_legal(&world, catalogs, &path, ROBOT_RADIUS);
    let surface_count = surface_waypoints_before_portal(&path).len();
    assert!(
        surface_count <= 4,
        "entrance-facing route should stay short (got {surface_count} surface waypoints)"
    );
}

#[test]
fn surface_to_approach_matches_pre_entrance_surface_routing_for_interior() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let interior_goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let portal = entrance_portal_for_building(&world, building_id);
    let approach = resolve_surface_entrance_approach_position(
        &world,
        world.space_registry(),
        &portal,
        ROBOT_RADIUS,
    )
    .expect("approach");
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let nav_config = NavigationConfig::default();

    let surface_only = find_path_with_spaces(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        approach,
        SpaceId::SURFACE,
        SpaceId::SURFACE,
        None,
    )
    .expect("surface-only path to approach");
    let to_interior = find_path_with_spaces(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        interior_goal,
        SpaceId::SURFACE,
        goal_space,
        None,
    )
    .expect("surface to interior path");

    assert_surface_path_legal(&world, catalogs, &surface_only, ROBOT_RADIUS);
    assert_surface_path_legal(&world, catalogs, &to_interior, ROBOT_RADIUS);

    let layout = world.layout();
    let approach_xz = approach.to_global(layout).xz();
    let interior_surface = surface_waypoints_before_portal(&to_interior);
    let surface_only_points: Vec<_> = surface_waypoints_before_portal(&surface_only)
        .iter()
        .map(|(_, waypoint)| waypoint.position.to_global(layout).xz())
        .collect();
    let interior_surface_points: Vec<_> = interior_surface
        .iter()
        .map(|(_, waypoint)| waypoint.position.to_global(layout).xz())
        .collect();

    assert!(
        interior_surface_points
            .iter()
            .any(|point| point.distance(approach_xz) < 1.0),
        "interior path must visit terrain-side approach"
    );
    assert!(
        surface_only_points
            .iter()
            .all(|point| interior_surface_points
                .iter()
                .any(|other| other.distance(*point) < 1.0)),
        "surface-only path waypoints must appear in pre-portal interior approach routing"
    );
}

#[test]
fn disabled_entrance_blocks_surface_to_interior_path() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let portal_id = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap()
        .portal_keys
        .get("exterior_entrance")
        .copied()
        .unwrap();
    world
        .space_registry_mut()
        .get_portal_mut(portal_id)
        .expect("portal")
        .enabled = false;

    let start = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);

    assert!(
        find_path_with_spaces(
            &world,
            catalogs,
            &NavigationConfig::default(),
            ROBOT_RADIUS,
            MAX_SLOPE,
            start,
            goal,
            SpaceId::SURFACE,
            goal_space,
            None,
        )
        .is_err(),
        "disabled entrance must not produce a traversable path"
    );
}

#[test]
fn too_large_agent_cannot_plan_surface_to_interior_approach() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = exterior_approach(&world, building_id);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    const OVERSIZED: f32 = 2.5;

    assert!(
        resolve_surface_entrance_approach_position(
            &world,
            world.space_registry(),
            &entrance_portal_for_building(&world, building_id),
            OVERSIZED,
        )
        .is_none(),
        "oversized agent must not resolve terrain-side approach"
    );
    assert!(
        find_path_with_spaces(
            &world,
            catalogs,
            &NavigationConfig::default(),
            OVERSIZED,
            MAX_SLOPE,
            start,
            goal,
            SpaceId::SURFACE,
            goal_space,
            None,
        )
        .is_err(),
        "oversized agent must not receive a cross-space interior path"
    );
}
