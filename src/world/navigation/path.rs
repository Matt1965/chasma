use bevy::prelude::*;

use super::waypoint::NavigationWaypoint;
use crate::world::{SpaceId, WorldPosition};

/// Authoritative navigation waypoints (ADR-032 U7, ADR-083 B6).
///
/// Grounded [`WorldPosition`] samples along an A* grid path with [`SpaceId`].
/// Stored on [`crate::world::UnitState::Moving`]; not exposed to ECS.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct NavigationPath {
    pub waypoints: Vec<NavigationWaypoint>,
}

impl NavigationPath {
    pub fn new(waypoints: Vec<NavigationWaypoint>) -> Self {
        Self { waypoints }
    }

    pub fn from_surface_positions(positions: Vec<WorldPosition>) -> Self {
        Self {
            waypoints: positions
                .into_iter()
                .map(NavigationWaypoint::surface)
                .collect(),
        }
    }

    pub fn positions(&self) -> impl Iterator<Item = WorldPosition> + '_ {
        self.waypoints.iter().map(|waypoint| waypoint.position)
    }

    /// Sum of XZ segment lengths in world meters.
    pub fn length_meters(&self, layout: crate::world::ChunkLayout) -> f32 {
        self.waypoints
            .windows(2)
            .map(|segment| xz_distance(segment[0].position, segment[1].position, layout))
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.waypoints.is_empty()
    }

    pub fn len(&self) -> usize {
        self.waypoints.len()
    }
}

/// Straight-line XZ distance between two authoritative positions (meters).
pub fn xz_distance(a: WorldPosition, b: WorldPosition, layout: crate::world::ChunkLayout) -> f32 {
    let a = a.to_global(layout);
    let b = b.to_global(layout);
    Vec2::new(b.x - a.x, b.z - a.z).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn from_surface_positions_sets_space_id() {
        let path = NavigationPath::from_surface_positions(vec![pos(1.0, 2.0)]);
        assert_eq!(path.waypoints[0].space_id, SpaceId::SURFACE);
    }
}
