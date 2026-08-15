//! Movement direction stabilization (ADR-037 U12).

use bevy::prelude::Vec2;

use crate::world::{ChunkLayout, NavigationPath, WorldPosition, xz_distance};

/// Distance below which a waypoint is treated as already reached for direction lock.
pub const WAYPOINT_DIRECTION_EPSILON_METERS: f32 = 0.25;

/// Resolved movement heading from the active path (never raw click target).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StabilizedMovementHeading {
    pub waypoint_index: usize,
    pub direction_xz: Vec2,
}

/// Compute the authoritative XZ direction toward the current path segment.
///
/// Skips waypoints already under the unit and never falls back to the move target.
pub fn stabilized_movement_heading(
    current: WorldPosition,
    path: &NavigationPath,
    waypoint_index: usize,
    layout: ChunkLayout,
) -> Option<StabilizedMovementHeading> {
    if path.is_empty() {
        return None;
    }

    let mut index = waypoint_index.min(path.len().saturating_sub(1));
    while index < path.len() {
        let waypoint_meta = &path.waypoints[index];
        if waypoint_meta.portal_id.is_some() {
            let waypoint = waypoint_meta.position;
            let direction = direction_toward(current, waypoint, layout);
            if direction.length_squared() > 1e-8 {
                return Some(StabilizedMovementHeading {
                    waypoint_index: index,
                    direction_xz: direction,
                });
            }
            break;
        }
        let waypoint = waypoint_meta.position;
        let distance = xz_distance(current, waypoint, layout);
        if distance <= WAYPOINT_DIRECTION_EPSILON_METERS && index + 1 < path.len() {
            index += 1;
            continue;
        }
        let direction = direction_toward(current, waypoint, layout);
        if direction.length_squared() > 1e-8 {
            return Some(StabilizedMovementHeading {
                waypoint_index: index,
                direction_xz: direction,
            });
        }
        break;
    }
    None
}

/// Whether steering may adjust the movement vector this tick.
pub fn steering_is_allowed(heading: Option<StabilizedMovementHeading>) -> bool {
    heading.is_some_and(|h| h.direction_xz.length_squared() > 1e-8)
}

/// When heading lookahead selects a later ordinary same-space waypoint, return the persistent
/// index that must be committed so movement cannot regress to an earlier waypoint.
pub fn heading_lookahead_commit_index(
    path: &NavigationPath,
    waypoint_index: usize,
    lookahead_index: usize,
) -> Option<usize> {
    if lookahead_index <= waypoint_index {
        return None;
    }
    for index in waypoint_index..lookahead_index {
        let wp = path.waypoints.get(index)?;
        if wp.portal_id.is_some() {
            return None;
        }
        let next = path.waypoints.get(index + 1)?;
        if next.portal_id.is_some() {
            return None;
        }
        if wp.space_id != next.space_id {
            return None;
        }
    }
    let target = path.waypoints.get(lookahead_index)?;
    if target.portal_id.is_some() {
        return None;
    }
    Some(lookahead_index)
}

fn direction_toward(from: WorldPosition, to: WorldPosition, layout: ChunkLayout) -> Vec2 {
    let from_global = from.to_global(layout);
    let to_global = to.to_global(layout);
    let delta = Vec2::new(to_global.x - from_global.x, to_global.z - from_global.z);
    if delta.length_squared() <= 1e-8 {
        return Vec2::ZERO;
    }
    delta.normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(bevy::prelude::Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn uses_first_non_consumed_waypoint_direction() {
        let path = NavigationPath::from_surface_positions(vec![pos(10.0, 10.0), pos(40.0, 10.0)]);
        let heading = stabilized_movement_heading(pos(10.0, 10.0), &path, 0, layout()).unwrap();
        assert_eq!(heading.waypoint_index, 1);
        assert!((heading.direction_xz.x - 1.0).abs() < 1e-4);
    }

    #[test]
    fn no_fallback_when_path_empty() {
        let path = NavigationPath::default();
        assert!(stabilized_movement_heading(pos(0.0, 0.0), &path, 0, layout()).is_none());
    }

    #[test]
    fn commit_index_accepts_ordinary_same_space_lookahead() {
        let path = NavigationPath::from_surface_positions(vec![pos(0.0, 0.0), pos(0.0, 20.0)]);
        assert_eq!(heading_lookahead_commit_index(&path, 0, 1), Some(1));
    }

    #[test]
    fn commit_index_rejects_portal_waypoint() {
        use crate::world::{NavigationWaypoint, PortalId, SpaceId};
        let portal = PortalId::new(1);
        let path = NavigationPath::new(vec![
            NavigationWaypoint::in_space(pos(0.0, 0.0), SpaceId::SURFACE),
            NavigationWaypoint::portal_transition(pos(1.0, 1.0), SpaceId::SURFACE, portal),
            NavigationWaypoint::in_space(pos(2.0, 2.0), SpaceId::new(1)),
        ]);
        assert_eq!(heading_lookahead_commit_index(&path, 0, 2), None);
    }

    #[test]
    fn commit_index_rejects_cross_space_without_portal() {
        use crate::world::{NavigationWaypoint, SpaceId};
        let path = NavigationPath::new(vec![
            NavigationWaypoint::in_space(pos(0.0, 0.0), SpaceId::SURFACE),
            NavigationWaypoint::in_space(pos(1.0, 1.0), SpaceId::new(1)),
        ]);
        assert_eq!(heading_lookahead_commit_index(&path, 0, 1), None);
    }
}
