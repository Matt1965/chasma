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
        let waypoint = path.waypoints[index];
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
        let path = NavigationPath::new(vec![pos(10.0, 10.0), pos(40.0, 10.0)]);
        let heading = stabilized_movement_heading(pos(10.0, 10.0), &path, 0, layout()).unwrap();
        assert_eq!(heading.waypoint_index, 1);
        assert!((heading.direction_xz.x - 1.0).abs() < 1e-4);
    }

    #[test]
    fn no_fallback_when_path_empty() {
        let path = NavigationPath::default();
        assert!(stabilized_movement_heading(pos(0.0, 0.0), &path, 0, layout()).is_none());
    }
}
