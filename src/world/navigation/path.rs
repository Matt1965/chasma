use bevy::prelude::*;

use crate::world::WorldPosition;

/// Authoritative navigation waypoints (ADR-032 U7).
///
/// Grounded [`WorldPosition`] samples along an A* grid path. Stored on
/// [`crate::world::UnitState::Moving`]; not exposed to ECS.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct NavigationPath {
    pub waypoints: Vec<WorldPosition>,
}

impl NavigationPath {
    pub fn new(waypoints: Vec<WorldPosition>) -> Self {
        Self { waypoints }
    }

    /// Sum of XZ segment lengths in world meters.
    pub fn length_meters(&self, layout: crate::world::ChunkLayout) -> f32 {
        self.waypoints
            .windows(2)
            .map(|segment| xz_distance(segment[0], segment[1], layout))
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
