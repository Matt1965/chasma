//! Conservative navigation path post-processing (ADR-032).

use bevy::prelude::*;

use super::grid::{
    NavigationAgent, NavigationConfig, grid_coord_at_position, is_cell_walkable,
    is_cell_walkable_in_space, is_position_walkable, is_position_walkable_in_space,
};
use crate::world::{
    ChunkLayout, PassabilityCatalogs, SpaceId, SpaceRegistry, WorldData, WorldPosition,
    ground_position_in_space, ground_world_position,
};

/// Remove collinear grid waypoints and apply greedy line-of-sight shortcuts (surface).
pub fn simplify_navigation_path(
    waypoints: &mut Vec<WorldPosition>,
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    layout: ChunkLayout,
) {
    if waypoints.len() <= 2 {
        return;
    }
    remove_collinear_waypoints(waypoints, layout);
    apply_line_of_sight_shortcuts(
        waypoints,
        world,
        catalogs,
        config.config_for_space(SpaceId::SURFACE),
        agent,
        layout,
        |from, to| {
            has_walkable_line_of_sight_surface(world, catalogs, config, agent, from, to, layout)
        },
    );
}

/// Space-aware path simplification (IN-03).
pub fn simplify_navigation_path_in_space(
    waypoints: &mut Vec<WorldPosition>,
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    layout: ChunkLayout,
) {
    if waypoints.len() <= 2 {
        return;
    }
    if space_id.is_surface() {
        simplify_navigation_path(waypoints, world, catalogs, config, agent, layout);
        return;
    }
    let space_config = config.config_for_space(space_id);
    remove_collinear_waypoints(waypoints, layout);
    apply_line_of_sight_shortcuts(
        waypoints,
        world,
        catalogs,
        space_config,
        agent,
        layout,
        |from, to| {
            is_segment_walkable_in_space(
                world,
                space_registry,
                catalogs,
                config,
                space_id,
                agent,
                from,
                to,
                layout,
            )
        },
    );
}

/// Whether every sample along `from`→`to` is walkable within `space_id`.
pub fn is_segment_walkable_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> bool {
    if space_id.is_surface() {
        return has_walkable_line_of_sight_surface(
            world, catalogs, config, agent, from, to, layout,
        );
    }

    if !crate::world::interior_segment_respects_region_boundary(
        world.building_navigation_runtime(),
        space_registry,
        layout,
        from,
        to,
        space_id,
        agent.radius_meters,
    ) {
        return false;
    }

    let space_config = config.config_for_space(space_id);
    let from_global = from.to_global(layout);
    let to_global = to.to_global(layout);
    let delta = Vec2::new(to_global.x - from_global.x, to_global.z - from_global.z);
    let distance = delta.length();
    if distance <= 1e-4 {
        return is_position_walkable_in_space(
            world,
            space_registry,
            catalogs,
            from,
            agent,
            space_id,
        );
    }

    let sample_spacing = space_config.cell_spacing_meters * 0.5;
    let steps = ((distance / sample_spacing).ceil() as usize).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let global = from_global.lerp(to_global, t);
        let candidate = WorldPosition::from_global(Vec3::new(global.x, 0.0, global.z), layout);
        let Some(grounded) = ground_position_in_space(world, space_registry, space_id, candidate)
        else {
            return false;
        };
        if !is_position_walkable_in_space(
            world,
            space_registry,
            catalogs,
            grounded,
            agent,
            space_id,
        ) {
            return false;
        }
        let cell = grid_coord_at_position(grounded, layout, space_config);
        if !is_cell_walkable_in_space(
            world,
            space_registry,
            catalogs,
            space_config,
            agent,
            cell,
            space_id,
        ) {
            return false;
        }
    }
    true
}

fn remove_collinear_waypoints(waypoints: &mut Vec<WorldPosition>, layout: ChunkLayout) {
    if waypoints.len() <= 2 {
        return;
    }
    let mut index = 0;
    while index + 2 < waypoints.len() {
        if is_collinear_xz(
            waypoints[index],
            waypoints[index + 1],
            waypoints[index + 2],
            layout,
        ) {
            waypoints.remove(index + 1);
        } else {
            index += 1;
        }
    }
}

fn apply_line_of_sight_shortcuts(
    waypoints: &mut Vec<WorldPosition>,
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    layout: ChunkLayout,
    mut line_of_sight: impl FnMut(WorldPosition, WorldPosition) -> bool,
) {
    if waypoints.len() <= 2 {
        return;
    }

    let _ = (world, catalogs, config, agent);

    let mut simplified = vec![waypoints[0]];
    let mut anchor = 0;
    while anchor < waypoints.len() - 1 {
        let mut best = anchor + 1;
        for probe in (anchor + 1..waypoints.len()).rev() {
            if line_of_sight(waypoints[anchor], waypoints[probe]) {
                best = probe;
                break;
            }
        }
        simplified.push(waypoints[best]);
        anchor = best;
    }

    *waypoints = simplified;
}

fn has_walkable_line_of_sight_surface(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> bool {
    let surface_config = config.config_for_space(SpaceId::SURFACE);
    let from_global = from.to_global(layout);
    let to_global = to.to_global(layout);
    let delta = Vec2::new(to_global.x - from_global.x, to_global.z - from_global.z);
    let distance = delta.length();
    if distance <= 1e-4 {
        return true;
    }

    let sample_spacing = surface_config.cell_spacing_meters * 0.5;
    let steps = ((distance / sample_spacing).ceil() as usize).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let global = from_global.lerp(to_global, t);
        let candidate = WorldPosition::from_global(Vec3::new(global.x, 0.0, global.z), layout);
        let Some(grounded) = ground_world_position(world, candidate) else {
            return false;
        };
        let cell = grid_coord_at_position(grounded, layout, surface_config);
        if !is_cell_walkable(world, catalogs, surface_config, agent, cell) {
            return false;
        }
        if !is_position_walkable(world, catalogs, grounded, agent) {
            return false;
        }
    }
    true
}

fn is_collinear_xz(
    a: WorldPosition,
    b: WorldPosition,
    c: WorldPosition,
    layout: ChunkLayout,
) -> bool {
    let a = a.to_global(layout);
    let b = b.to_global(layout);
    let c = c.to_global(layout);
    let ab = Vec2::new(b.x - a.x, b.z - a.z);
    let bc = Vec2::new(c.x - b.x, c.z - b.z);
    if ab.length_squared() < 1e-6 || bc.length_squared() < 1e-6 {
        return true;
    }
    (ab.x * bc.y - ab.y * bc.x).abs() < 1e-4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, Heightfield, LocalPosition};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
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

    #[test]
    fn collinear_points_are_removed() {
        let mut waypoints = vec![pos(4.0, 4.0), pos(12.0, 12.0), pos(20.0, 20.0)];
        remove_collinear_waypoints(&mut waypoints, layout());
        assert_eq!(waypoints.len(), 2);
    }
}
