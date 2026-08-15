//! Agent-clearance-safe Interior planning anchors for exterior entrances (IN-11gI-C).

use bevy::prelude::*;

use super::grid::{NavigationAgent, NavigationConfig};
use super::legality::{
    NavigationSegmentLegality, query_navigation_point_legality, query_navigation_segment_legality,
};
use crate::world::occupancy::{PassabilityAgent, PassabilityCatalogs, PassabilityResult};
use crate::world::{
    ChunkLayout, PortalRecord, PortalType, SpaceId, SpaceRegistry, WorldData, WorldPosition,
    ground_position_in_space, surface_segment_respects_blueprint_boundaries,
};

const INWARD_SEARCH_STEP_METERS: f32 = 0.1;
const INWARD_SEARCH_MAX_STEPS: usize = 48;
const EXTERIOR_CROSSING_OFFSET_METERS: f32 = 1.0;

/// Resolve a universal-legal Interior continuation anchor for an exterior entrance portal.
///
/// Returns the authored landing when it already passes point legality for `agent`.
/// Otherwise searches farther inward along the entrance inward normal.
pub fn resolve_entrance_interior_planning_anchor(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    portal: &PortalRecord,
    dest_space: SpaceId,
    layout: ChunkLayout,
    config: NavigationConfig,
    agent: NavigationAgent,
) -> Option<WorldPosition> {
    let authored = ground_position_in_space(world, space_registry, dest_space, portal.to_position)?;
    if portal.portal_type != PortalType::ExteriorEntrance {
        return Some(authored);
    }
    let Some(threshold) = portal.entrance_threshold_global_xz else {
        return point_legal_anchor(
            world,
            space_registry,
            catalogs,
            portal,
            dest_space,
            layout,
            config,
            agent,
            authored,
            None,
        )
        .then_some(authored);
    };

    if point_legal_anchor(
        world,
        space_registry,
        catalogs,
        portal,
        dest_space,
        layout,
        config,
        agent,
        authored,
        Some(threshold),
    ) {
        return Some(authored);
    }

    let landing_xz = portal.to_position.to_global(layout).xz();
    let inward = landing_xz - threshold;
    if inward.length_squared() <= f32::EPSILON {
        return None;
    }
    let inward = inward.normalize();

    for step in 1..=INWARD_SEARCH_MAX_STEPS {
        let candidate_xz = landing_xz + inward * (step as f32 * INWARD_SEARCH_STEP_METERS);
        let candidate =
            WorldPosition::from_global(Vec3::new(candidate_xz.x, 0.0, candidate_xz.y), layout);
        let Some(grounded) = ground_position_in_space(world, space_registry, dest_space, candidate)
        else {
            continue;
        };
        if point_legal_anchor(
            world,
            space_registry,
            catalogs,
            portal,
            dest_space,
            layout,
            config,
            agent,
            grounded,
            Some(threshold),
        ) {
            return Some(grounded);
        }
    }
    None
}

fn point_legal_anchor(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    portal: &PortalRecord,
    dest_space: SpaceId,
    layout: ChunkLayout,
    config: NavigationConfig,
    agent: NavigationAgent,
    position: WorldPosition,
    threshold: Option<Vec2>,
) -> bool {
    let passability_agent = PassabilityAgent::from(agent);
    if !matches!(
        query_navigation_point_legality(world, catalogs, position, passability_agent, dest_space),
        PassabilityResult::Passable { .. }
    ) {
        return false;
    }
    if portal.portal_type != PortalType::ExteriorEntrance {
        return true;
    }
    let Some(threshold) = threshold else {
        return true;
    };
    entrance_crossing_legal(
        world,
        space_registry,
        catalogs,
        portal,
        dest_space,
        layout,
        config,
        agent,
        position,
        threshold,
    )
}

fn entrance_crossing_legal(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    portal: &PortalRecord,
    dest_space: SpaceId,
    layout: ChunkLayout,
    config: NavigationConfig,
    agent: NavigationAgent,
    candidate: WorldPosition,
    threshold: Vec2,
) -> bool {
    let landing_xz = portal.to_position.to_global(layout).xz();
    let inward = landing_xz - threshold;
    if inward.length_squared() <= f32::EPSILON {
        return false;
    }
    let inward = inward.normalize();
    let outward = -inward;

    let exterior_xz = threshold + outward * EXTERIOR_CROSSING_OFFSET_METERS;
    let from_surface =
        WorldPosition::from_global(Vec3::new(exterior_xz.x, 0.0, exterior_xz.y), layout);
    let Some(from_surface) =
        ground_position_in_space(world, space_registry, SpaceId::SURFACE, from_surface)
    else {
        return false;
    };
    if !surface_segment_respects_blueprint_boundaries(
        world,
        from_surface,
        candidate,
        layout,
        agent.radius_meters,
    ) {
        return false;
    }

    let Some(from_interior) = legal_interior_segment_start(
        world,
        space_registry,
        catalogs,
        dest_space,
        layout,
        agent,
        threshold,
        inward,
    ) else {
        return false;
    };
    matches!(
        query_navigation_segment_legality(
            world,
            space_registry,
            catalogs,
            config,
            dest_space,
            agent,
            from_interior,
            candidate,
            layout,
        ),
        NavigationSegmentLegality::Legal
    )
}

/// First universal-legal point inward from the entrance threshold for segment validation.
///
/// Skips inset `0`: a threshold-on-edge point may be point-legal through an opening but still
/// produces a segment that geometrically departs across the owning boundary edge.
fn legal_interior_segment_start(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    dest_space: SpaceId,
    layout: ChunkLayout,
    agent: NavigationAgent,
    threshold: Vec2,
    inward: Vec2,
) -> Option<WorldPosition> {
    let passability_agent = PassabilityAgent::from(agent);
    for step in 1..=INWARD_SEARCH_MAX_STEPS {
        let inset = step as f32 * INWARD_SEARCH_STEP_METERS;
        let inside_xz = threshold + inward * inset;
        let probe = WorldPosition::from_global(Vec3::new(inside_xz.x, 0.0, inside_xz.y), layout);
        let Some(grounded) = ground_position_in_space(world, space_registry, dest_space, probe)
        else {
            continue;
        };
        if matches!(
            query_navigation_point_legality(
                world,
                catalogs,
                grounded,
                passability_agent,
                dest_space,
            ),
            PassabilityResult::Passable { .. }
        ) {
            return Some(grounded);
        }
    }
    None
}
