//! Dev-only LEG 1 diagnostics for Interior → Surface cross-space routing (IN-11gI-E-T3).
//!
//! Read-only observation around `find_path_single_space` for reverse exterior-entrance approach.

#[cfg(feature = "dev")]
use std::cell::RefCell;

#[cfg(feature = "dev")]
use bevy::prelude::*;

#[cfg(feature = "dev")]
use super::entrance_interior_anchor::resolve_entrance_interior_planning_anchor;
#[cfg(feature = "dev")]
use super::grid::{NavigationAgent, NavigationConfig};
#[cfg(feature = "dev")]
use super::query::NavigationError;
#[cfg(feature = "dev")]
use crate::world::occupancy::{PassabilityAgent, PassabilityBlockReason, PassabilityResult};
#[cfg(feature = "dev")]
use crate::world::{
    PortalId, PortalRecord, PortalType, SpaceId, SpaceRegistry, WorldData, WorldPosition,
    ground_position_in_space, interior_position_walkable, min_edge_clearance_meters,
    query_navigation_point_legality,
};

#[cfg(feature = "dev")]
thread_local! {
    static PENDING_LEG_TRACE_LINES: RefCell<Option<Vec<String>>> = RefCell::new(None);
}

#[cfg(feature = "dev")]
pub fn take_pending_leg_trace_lines() -> Option<Vec<String>> {
    PENDING_LEG_TRACE_LINES.with(|pending| pending.borrow_mut().take())
}

#[cfg(feature = "dev")]
pub fn should_trace_reverse_interior_leg1(
    route_start_space: SpaceId,
    route_goal_space: SpaceId,
    leg_space: SpaceId,
    portal: &PortalRecord,
) -> bool {
    !route_start_space.is_surface()
        && route_goal_space.is_surface()
        && !leg_space.is_surface()
        && portal.portal_type == PortalType::ExteriorEntrance
}

#[cfg(feature = "dev")]
pub fn record_reverse_interior_leg1_failure(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: crate::world::PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent: NavigationAgent,
    route_start_space: SpaceId,
    route_goal_space: SpaceId,
    leg_index: u32,
    portal_id: PortalId,
    portal: &PortalRecord,
    leg_space: SpaceId,
    leg_start: WorldPosition,
    leg_goal: WorldPosition,
    error: NavigationError,
) {
    let lines = build_leg_trace_lines(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        route_start_space,
        route_goal_space,
        leg_index,
        portal_id,
        portal,
        leg_space,
        leg_start,
        leg_goal,
        error,
    );
    if world.interior_exit_click_trace().has_active_session() {
        PENDING_LEG_TRACE_LINES.with(|pending| *pending.borrow_mut() = Some(lines));
    } else {
        crate::logging::append_log_block(
            crate::logging::NAVIGATION_TRACE_LOG_PATH,
            "# chasma navigation trace",
            &lines.join("\n"),
        );
    }
}

#[cfg(feature = "dev")]
fn build_leg_trace_lines(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: crate::world::PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent: NavigationAgent,
    route_start_space: SpaceId,
    route_goal_space: SpaceId,
    leg_index: u32,
    portal_id: PortalId,
    portal: &PortalRecord,
    leg_space: SpaceId,
    leg_start: WorldPosition,
    leg_goal: WorldPosition,
    error: NavigationError,
) -> Vec<String> {
    let layout = world.layout();
    let passability_agent = PassabilityAgent::from(agent);
    let runtime = world.building_navigation_runtime();

    let grounded_start =
        ground_position_in_space(world, space_registry, leg_space, leg_start).unwrap_or(leg_start);
    let grounded_goal =
        ground_position_in_space(world, space_registry, leg_space, leg_goal).unwrap_or(leg_goal);

    let start_probe = probe_point_legality(
        world,
        catalogs,
        grounded_start,
        passability_agent,
        leg_space,
    );
    let goal_probe =
        probe_point_legality(world, catalogs, grounded_goal, passability_agent, leg_space);

    let goal_xz = grounded_goal.to_global(layout).xz();
    let inside_region =
        interior_position_walkable(runtime, space_registry, layout, grounded_goal, leg_space);
    let min_boundary_clearance = runtime
        .region_for_space(leg_space)
        .map(|region| min_edge_clearance_meters(goal_xz, &region.world_outline_xz));

    let raw_landing =
        ground_position_in_space(world, space_registry, leg_space, portal.to_position);
    let raw_probe = raw_landing.map(|position| {
        probe_point_legality(world, catalogs, position, passability_agent, leg_space)
    });
    let resolved_anchor = resolve_entrance_interior_planning_anchor(
        world,
        space_registry,
        catalogs,
        portal,
        leg_space,
        layout,
        *config,
        agent,
    );
    let anchor_probe = resolved_anchor.map(|position| {
        probe_point_legality(world, catalogs, position, passability_agent, leg_space)
    });

    let mut lines = vec![
        "[CROSS_SPACE_LEG_TRACE]".to_string(),
        "route_direction=InteriorToSurface".to_string(),
        format!("leg_index={leg_index}"),
        format!("portal_id={}", portal_id.raw()),
        format!("leg_space={}", leg_space.raw()),
        format!("leg_start={}", format_pos(grounded_start, layout)),
        format!("leg_goal={}", format_pos(grounded_goal, layout)),
        format!(
            "leg_goal_source={}",
            leg_goal_source(portal, leg_space, layout, grounded_goal)
        ),
        format!("leg_start_legality={}", start_probe.legality),
        format!(
            "leg_start_block_reason={}",
            start_probe.block_reason.as_deref().unwrap_or("none")
        ),
        format!(
            "leg_start_unavailable_reason={}",
            start_probe.unavailable_reason.as_deref().unwrap_or("none")
        ),
        format!("leg_goal_legality={}", goal_probe.legality),
        format!(
            "leg_goal_block_reason={}",
            goal_probe.block_reason.as_deref().unwrap_or("none")
        ),
        format!(
            "leg_goal_unavailable_reason={}",
            goal_probe.unavailable_reason.as_deref().unwrap_or("none")
        ),
        format!("agent_radius_m={:.3}", agent.radius_meters),
        format!("goal_inside_region={inside_region}"),
        format!(
            "goal_min_boundary_clearance_m={}",
            min_boundary_clearance
                .map(|v| format!("{v:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("goal_required_clearance_m={:.3}", agent.radius_meters),
        format!(
            "raw_interior_landing={}",
            raw_landing
                .map(|p| format_pos(p, layout))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "raw_landing_legality={}",
            raw_probe
                .map(|p| p.legality)
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "resolved_safe_anchor={}",
            resolved_anchor
                .map(|p| format_pos(p, layout))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "resolved_safe_anchor_legality={}",
            anchor_probe
                .map(|p| p.legality)
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("find_path_single_space_result=error"),
        format!("returned_navigation_error={}", format_nav_error(error)),
        format!("route_start_space={}", route_start_space.raw()),
        format!("route_goal_space={}", route_goal_space.raw()),
    ];
    lines
}

#[cfg(feature = "dev")]
struct PointLegalityProbe {
    legality: String,
    block_reason: Option<String>,
    unavailable_reason: Option<String>,
}

#[cfg(feature = "dev")]
fn probe_point_legality(
    world: &WorldData,
    catalogs: crate::world::PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
    space_id: SpaceId,
) -> PointLegalityProbe {
    let result = query_navigation_point_legality(world, catalogs, position, agent, space_id);
    PointLegalityProbe {
        legality: legality_label(&result),
        block_reason: block_reason_label(&result),
        unavailable_reason: unavailable_reason_label(&result),
    }
}

#[cfg(feature = "dev")]
fn leg_goal_source(
    portal: &PortalRecord,
    leg_space: SpaceId,
    layout: crate::world::ChunkLayout,
    leg_goal: WorldPosition,
) -> String {
    if portal.portal_type == PortalType::ExteriorEntrance
        && portal.bidirectional
        && portal.to_space == leg_space
    {
        let trigger_xz = portal.to_position.to_global(layout).xz();
        let goal_xz = leg_goal.to_global(layout).xz();
        if goal_xz.distance(trigger_xz) < 0.05 {
            return "portal_to_position_trigger".to_string();
        }
        return "portal_to_position_trigger_offset".to_string();
    }
    if portal.from_space == leg_space {
        let trigger_xz = portal.from_center_global_xz;
        let goal_xz = leg_goal.to_global(layout).xz();
        if goal_xz.distance(trigger_xz) < 0.05 {
            return "portal_from_center_trigger".to_string();
        }
        return "portal_from_center_trigger_offset".to_string();
    }
    "other".to_string()
}

#[cfg(feature = "dev")]
fn format_pos(position: WorldPosition, layout: crate::world::ChunkLayout) -> String {
    let g = position.to_global(layout);
    format!("({:.2},{:.2},{:.2})", g.x, g.y, g.z)
}

#[cfg(feature = "dev")]
fn legality_label(result: &PassabilityResult) -> String {
    match result {
        PassabilityResult::Passable { .. } => "Passable".to_string(),
        PassabilityResult::Blocked { .. } => "Blocked".to_string(),
        PassabilityResult::Unavailable { .. } => "Unavailable".to_string(),
    }
}

#[cfg(feature = "dev")]
fn block_reason_label(result: &PassabilityResult) -> Option<String> {
    match result {
        PassabilityResult::Blocked { reason, .. } => Some(format_block_reason(*reason)),
        _ => None,
    }
}

#[cfg(feature = "dev")]
fn unavailable_reason_label(result: &PassabilityResult) -> Option<String> {
    match result {
        PassabilityResult::Unavailable { reason } => Some(format_unavailable_reason(*reason)),
        _ => None,
    }
}

#[cfg(feature = "dev")]
fn format_block_reason(reason: PassabilityBlockReason) -> String {
    match reason {
        PassabilityBlockReason::SlopeTooSteep => "SlopeTooSteep".to_string(),
        PassabilityBlockReason::BuildingOccupied => "BuildingOccupied".to_string(),
        PassabilityBlockReason::DoodadOccupied => "DoodadOccupied".to_string(),
        PassabilityBlockReason::CorruptFootprint => "CorruptFootprint".to_string(),
        PassabilityBlockReason::MissingDefinition => "MissingDefinition".to_string(),
        PassabilityBlockReason::InvalidCell => "InvalidCell".to_string(),
        PassabilityBlockReason::AgentClearanceInsufficient => {
            "AgentClearanceInsufficient".to_string()
        }
        PassabilityBlockReason::BlueprintSupport => "BlueprintSupport".to_string(),
    }
}

#[cfg(feature = "dev")]
fn format_unavailable_reason(
    reason: crate::world::occupancy::PassabilityUnavailableReason,
) -> String {
    match reason {
        crate::world::occupancy::PassabilityUnavailableReason::TerrainUnavailable => {
            "TerrainUnavailable".to_string()
        }
    }
}

#[cfg(feature = "dev")]
fn format_nav_error(error: NavigationError) -> String {
    match error {
        NavigationError::StartBlocked => "StartBlocked".to_string(),
        NavigationError::GoalBlocked => "GoalBlocked".to_string(),
        NavigationError::NoPath => "NoPath".to_string(),
        NavigationError::TerrainUnavailable => "TerrainUnavailable".to_string(),
    }
}

#[cfg(all(test, feature = "dev"))]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState,
        BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
        BuildingNavigationBlueprintInstanceOverride, BuildingOwnership, BuildingSource, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog, Heightfield,
        InteriorProfileCatalog, LocalPosition, NavigationEntranceDefinition,
        NavigationFloorDefinition, NavigationPolygon2d, NavigationRegionDefinition,
        place_player_building, set_building_lifecycle_stage,
    };
    use bevy::prelude::Quat;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
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

    fn activate_fixture(
        world: &mut WorldData,
        blueprint: BuildingNavigationBlueprint,
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
        let placement = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(80.0, 0.0, 80.0)),
        );
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

    #[test]
    fn leg_trace_reports_agent_clearance_on_tight_landing() {
        let mut world = flat_world();
        let building_id = activate_fixture(&mut world, oversized_concave_hut_blueprint());
        let runtime = world
            .building_navigation_runtime()
            .get(building_id)
            .expect("runtime");
        let interior_space = runtime.regions[0].space_id;
        let portal_id = *runtime
            .portal_keys
            .get("exterior_entrance")
            .expect("portal");
        let portal = world
            .space_registry()
            .get_portal(portal_id)
            .expect("portal");
        let layout = world.layout();
        let floor_y = world
            .space_registry()
            .get_space(interior_space)
            .expect("space")
            .floor_y_global;
        let landing_global = runtime
            .model_transform
            .transform_point(Vec3::new(7.0, 1.27, 1.5));
        let leg_goal = WorldPosition::from_global(
            Vec3::new(landing_global.x, floor_y, landing_global.z),
            layout,
        );
        let leg_start = WorldPosition::from_global(
            Vec3::new(landing_global.x - 4.0, floor_y, landing_global.z + 4.0),
            layout,
        );
        let catalogs = crate::world::PassabilityCatalogs {
            doodad: &DoodadCatalog::default(),
            building: &BuildingCatalog::default(),
            footprint: &FootprintCatalog::default(),
        };
        let agent = NavigationAgent {
            radius_meters: 0.68,
            max_slope_degrees: 40.0,
        };
        let lines = build_leg_trace_lines(
            &world,
            world.space_registry(),
            catalogs,
            &NavigationConfig::default(),
            agent,
            interior_space,
            SpaceId::SURFACE,
            1,
            portal_id,
            portal,
            interior_space,
            leg_start,
            leg_goal,
            NavigationError::GoalBlocked,
        );
        let joined = lines.join("\n");
        assert!(joined.contains("[CROSS_SPACE_LEG_TRACE]"));
        assert!(joined.contains("route_direction=InteriorToSurface"));
        assert!(joined.contains("leg_index=1"));
        assert!(joined.contains("leg_goal_source=portal_to_position_trigger"));
        assert!(joined.contains("leg_start_legality="));
        assert!(joined.contains("leg_goal_legality="));
        assert!(joined.contains("raw_interior_landing="));
        assert!(joined.contains("resolved_safe_anchor="));
        assert!(joined.contains("find_path_single_space_result=error"));
        assert!(joined.contains("returned_navigation_error=GoalBlocked"));
        assert!(joined.contains("goal_inside_region="));
        assert!(joined.contains("goal_min_boundary_clearance_m="));
    }
}
