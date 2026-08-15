//! Blueprint-derived Surface support exclusion and Entrance access corridors (NAV-GROUND-1).
//!
//! Hydrated blueprint-controlled buildings block ordinary Surface navigation inside the
//! horizontal projection of Interior navigation regions, except through aperture-aligned
//! access corridors at enabled exterior entrances.

use bevy::prelude::*;

use super::opening_geometry::{
    EdgeParametricInterval, point_parametric_on_edge, usable_center_opening_interval_on_edge,
};
use super::runtime::{RuntimeNavigationRegion, point_in_polygon_xz};
use crate::world::space::{PortalRecord, PortalType, SpaceRegistry};
use crate::world::{
    BuildingId, ChunkLayout, SpaceId, WorldData, WorldPosition, ground_position_in_space,
};

const CORRIDOR_DEPTH_STEP_METERS: f32 = 0.25;
const CORRIDOR_DEPTH_MAX_STEPS: usize = 160;
const CORRIDOR_OUTSIDE_MARGIN_METERS: f32 = 0.5;
const MIN_CORRIDOR_OUTWARD_DEPTH_METERS: f32 = 4.0;

/// Whether ordinary Surface navigation is blocked by blueprint building support at `point_xz`.
///
/// Ghost buildings (no hydrated runtime) never block. Points inside a region projection are
/// blocked unless they lie in an enabled Entrance access corridor for `agent_radius_meters`.
pub fn surface_blueprint_support_blocks_position(
    world: &WorldData,
    layout: ChunkLayout,
    point_xz: Vec2,
    agent_radius_meters: f32,
) -> Option<BuildingId> {
    let store = world.building_navigation_runtime();
    let space_registry = world.space_registry();
    let mut runtimes: Vec<_> = store.iter().collect();
    runtimes.sort_by_key(|runtime| runtime.building_id.raw());

    for runtime in runtimes {
        let building_id = runtime.building_id;
        for region in &runtime.regions {
            if !point_in_polygon_xz(&region.world_outline_xz, point_xz) {
                continue;
            }
            if surface_position_in_entrance_access_corridor(
                space_registry,
                layout,
                building_id,
                region,
                point_xz,
                agent_radius_meters,
            ) {
                continue;
            }
            return Some(building_id);
        }
    }
    None
}

/// Whether `point_xz` lies in an outward access corridor for any enabled exterior entrance on
/// `region`.
pub fn surface_position_in_entrance_access_corridor(
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    building_id: BuildingId,
    region: &RuntimeNavigationRegion,
    point_xz: Vec2,
    agent_radius_meters: f32,
) -> bool {
    for (_portal_id, portal) in space_registry.portals() {
        if !entrance_corridor_applies(portal, building_id, region.space_id) {
            continue;
        }
        if point_in_entrance_access_corridor_for_portal(
            point_xz,
            &region.world_outline_xz,
            portal,
            layout,
            agent_radius_meters,
        ) {
            return true;
        }
    }
    false
}

fn entrance_corridor_applies(
    portal: &PortalRecord,
    building_id: BuildingId,
    region_space: SpaceId,
) -> bool {
    portal.owning_building_id == Some(building_id)
        && portal.enabled
        && portal.portal_type == PortalType::ExteriorEntrance
        && portal.to_space == region_space
}

fn point_in_entrance_access_corridor_for_portal(
    point_xz: Vec2,
    polygon: &[Vec2],
    portal: &PortalRecord,
    layout: ChunkLayout,
    agent_radius_meters: f32,
) -> bool {
    let Some(edge_index) = portal.entrance_owning_edge_index else {
        return false;
    };
    let Some(threshold) = portal.entrance_threshold_global_xz else {
        return false;
    };
    if polygon.len() < 2 || edge_index as usize >= polygon.len() {
        return false;
    }
    let edge_index = edge_index as usize;
    let a = polygon[edge_index];
    let b = polygon[(edge_index + 1) % polygon.len()];
    let Some(interval) = usable_center_opening_interval_on_edge(
        a,
        b,
        threshold,
        portal.from_radius_meters,
        agent_radius_meters,
    ) else {
        return false;
    };

    let landing_xz = portal.to_position.to_global(layout).xz();
    let inward = landing_xz - threshold;
    if inward.length_squared() <= f32::EPSILON {
        return false;
    }
    let outward = -inward.normalize();

    let edge = b - a;
    let edge_len = edge.length();
    if edge_len <= f32::EPSILON {
        return false;
    }
    let edge_dir = edge / edge_len;
    let mut outward_normal = Vec2::new(-edge_dir.y, edge_dir.x);
    if outward_normal.dot(outward) < 0.0 {
        outward_normal = -outward_normal;
    }

    let Some(t) = point_parametric_on_edge(point_xz, a, b) else {
        return false;
    };
    if !interval.contains_t(t) {
        return false;
    }

    let edge_point = a + edge * t;
    let outward_distance = (point_xz - edge_point).dot(outward_normal);
    if outward_distance < -super::ENTRANCE_BOUNDARY_TOLERANCE {
        return false;
    }

    let max_depth = corridor_outward_depth_meters(polygon, a, b, interval, outward_normal);
    outward_distance <= max_depth
}

fn corridor_outward_depth_meters(
    polygon: &[Vec2],
    a: Vec2,
    b: Vec2,
    interval: EdgeParametricInterval,
    outward: Vec2,
) -> f32 {
    let edge = b - a;
    let origin = a + edge * ((interval.t_start + interval.t_end) * 0.5);
    let mut last_inside_depth = 0.0;
    for step in 1..=CORRIDOR_DEPTH_MAX_STEPS {
        let depth = step as f32 * CORRIDOR_DEPTH_STEP_METERS;
        let probe = origin + outward * depth;
        if point_in_polygon_xz(polygon, probe) {
            last_inside_depth = depth;
        } else {
            return (depth + CORRIDOR_OUTSIDE_MARGIN_METERS).max(MIN_CORRIDOR_OUTWARD_DEPTH_METERS);
        }
    }
    last_inside_depth + CORRIDOR_OUTSIDE_MARGIN_METERS.max(MIN_CORRIDOR_OUTWARD_DEPTH_METERS)
}

/// Global XZ at the terrain-side extent of the ExteriorEntrance access corridor (NAV-GROUND).
///
/// Shared ingress/egress geometry: outward extent of the corridor, outside the raw support
/// polygon, only when the agent fits the usable entrance aperture and the portal is enabled.
pub fn surface_entrance_terrain_side_corridor_global_xz(
    portal: &PortalRecord,
    region_outline: &[Vec2],
    layout: ChunkLayout,
    agent_radius_meters: f32,
) -> Option<Vec2> {
    if !portal.enabled || portal.portal_type != PortalType::ExteriorEntrance {
        return None;
    }
    let Some(edge_index) = portal.entrance_owning_edge_index else {
        return None;
    };
    let Some(threshold) = portal.entrance_threshold_global_xz else {
        return None;
    };
    if region_outline.len() < 2 || edge_index as usize >= region_outline.len() {
        return None;
    }
    let edge_index = edge_index as usize;
    let a = region_outline[edge_index];
    let b = region_outline[(edge_index + 1) % region_outline.len()];
    let Some(interval) = usable_center_opening_interval_on_edge(
        a,
        b,
        threshold,
        portal.from_radius_meters,
        agent_radius_meters,
    ) else {
        return None;
    };

    let landing_xz = portal.to_position.to_global(layout).xz();
    let inward = landing_xz - threshold;
    if inward.length_squared() <= f32::EPSILON {
        return None;
    }
    let outward = -inward.normalize();

    let edge = b - a;
    let edge_len = edge.length();
    if edge_len <= f32::EPSILON {
        return None;
    }
    let edge_dir = edge / edge_len;
    let mut outward_normal = Vec2::new(-edge_dir.y, edge_dir.x);
    if outward_normal.dot(outward) < 0.0 {
        outward_normal = -outward_normal;
    }

    let origin = a + edge * ((interval.t_start + interval.t_end) * 0.5);
    let max_depth = corridor_outward_depth_meters(region_outline, a, b, interval, outward_normal);
    let escape_xz = origin + outward_normal * max_depth;

    if point_in_polygon_xz(region_outline, escape_xz) {
        return None;
    }
    if !point_in_entrance_access_corridor_for_portal(
        escape_xz,
        region_outline,
        portal,
        layout,
        agent_radius_meters,
    ) {
        return None;
    }
    Some(escape_xz)
}

/// Global XZ of the terrain-side escape point for reverse ExteriorEntrance egress (NAV-EXIT).
pub fn surface_entrance_terrain_side_escape_global_xz(
    portal: &PortalRecord,
    region_outline: &[Vec2],
    layout: ChunkLayout,
    agent_radius_meters: f32,
) -> Option<Vec2> {
    surface_entrance_terrain_side_corridor_global_xz(
        portal,
        region_outline,
        layout,
        agent_radius_meters,
    )
}

/// Grounded Surface position at the shared terrain-side Entrance corridor extent.
pub fn resolve_surface_entrance_terrain_side_corridor_position(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    portal: &PortalRecord,
    agent_radius_meters: f32,
) -> Option<WorldPosition> {
    let building_id = portal.owning_building_id?;
    let runtime = world.building_navigation_runtime().get(building_id)?;
    let region = runtime
        .regions
        .iter()
        .find(|region| region.space_id == portal.to_space)?;
    let corridor_xz = surface_entrance_terrain_side_corridor_global_xz(
        portal,
        &region.world_outline_xz,
        world.layout(),
        agent_radius_meters,
    )?;
    let global = Vec3::new(corridor_xz.x, 0.0, corridor_xz.y);
    let position = WorldPosition::from_global(global, world.layout());
    ground_position_in_space(world, space_registry, SpaceId::SURFACE, position)
}

/// Grounded Surface position at the terrain-side escape point for reverse ExteriorEntrance egress.
pub fn resolve_surface_entrance_escape_position(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    portal: &PortalRecord,
    agent_radius_meters: f32,
) -> Option<WorldPosition> {
    resolve_surface_entrance_terrain_side_corridor_position(
        world,
        space_registry,
        portal,
        agent_radius_meters,
    )
}

/// Grounded Surface position at the terrain-side approach point for Surface→Interior ingress.
pub fn resolve_surface_entrance_approach_position(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    portal: &PortalRecord,
    agent_radius_meters: f32,
) -> Option<WorldPosition> {
    resolve_surface_entrance_terrain_side_corridor_position(
        world,
        space_registry,
        portal,
        agent_radius_meters,
    )
}
