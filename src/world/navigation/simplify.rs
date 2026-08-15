//! Conservative navigation path post-processing (ADR-032).

use bevy::prelude::*;

use super::grid::NavigationAgent;
use super::legality::query_navigation_segment_legality;
use crate::world::{
    ChunkLayout, PassabilityCatalogs, SpaceId, SpaceRegistry, WorldData, WorldPosition,
};

/// Remove collinear grid waypoints and apply greedy line-of-sight shortcuts (surface).
pub fn simplify_navigation_path(
    waypoints: &mut Vec<WorldPosition>,
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: super::grid::NavigationConfig,
    agent: NavigationAgent,
    layout: ChunkLayout,
) {
    simplify_navigation_path_in_space(
        waypoints,
        world,
        world.space_registry(),
        catalogs,
        config,
        SpaceId::SURFACE,
        agent,
        layout,
    );
}

/// Space-aware path simplification (IN-03).
pub fn simplify_navigation_path_in_space(
    waypoints: &mut Vec<WorldPosition>,
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: super::grid::NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    layout: ChunkLayout,
) {
    if waypoints.len() <= 2 {
        return;
    }
    let _space_config = config.config_for_space(space_id);
    remove_collinear_waypoints(waypoints, layout);
    apply_line_of_sight_shortcuts(
        waypoints,
        world,
        space_registry,
        catalogs,
        config,
        space_id,
        agent,
        layout,
        |from, to| {
            query_navigation_segment_legality(
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
            .is_legal()
        },
    );
}

/// Whether every sample along `from`→`to` is walkable within `space_id`.
///
/// Thin adapter over [`query_navigation_segment_legality`] (IN-11gF).
pub fn is_segment_walkable_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: super::grid::NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> bool {
    navigation_segment_valid(
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
}

/// Consolidated segment validity for movement and pathfinding (IN-11g).
///
/// Thin adapter over [`query_navigation_segment_legality`] (IN-11gF).
pub fn navigation_segment_valid(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: super::grid::NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> bool {
    query_navigation_segment_legality(
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
    .is_legal()
}

/// Whether every consecutive waypoint pair is universally legal in `space_id`.
pub fn all_consecutive_segments_legal_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: super::grid::NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    waypoints: &[WorldPosition],
    layout: ChunkLayout,
) -> bool {
    waypoints.windows(2).all(|pair| {
        query_navigation_segment_legality(
            world,
            space_registry,
            catalogs,
            config,
            space_id,
            agent,
            pair[0],
            pair[1],
            layout,
        )
        .is_legal()
    })
}

/// Thin adapter: surface line-of-sight uses universal segment legality (IN-11gG).
pub fn has_walkable_line_of_sight_surface(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: super::grid::NavigationConfig,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> bool {
    navigation_segment_valid(
        world,
        world.space_registry(),
        catalogs,
        config,
        SpaceId::SURFACE,
        agent,
        from,
        to,
        layout,
    )
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
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: super::grid::NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    _layout: ChunkLayout,
    mut segment_legal: impl FnMut(WorldPosition, WorldPosition) -> bool,
) {
    if waypoints.len() <= 2 {
        return;
    }

    let _ = (world, space_registry, catalogs, config, space_id, agent);

    let mut simplified = vec![waypoints[0]];
    let mut anchor = 0;
    while anchor < waypoints.len() - 1 {
        let mut best = anchor + 1;
        for probe in (anchor + 1..waypoints.len()).rev() {
            if segment_legal(waypoints[anchor], waypoints[probe]) {
                best = probe;
                break;
            }
        }
        simplified.push(waypoints[best]);
        anchor = best;
    }

    *waypoints = simplified;
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
