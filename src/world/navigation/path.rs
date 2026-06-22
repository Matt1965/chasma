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

    pub fn is_empty(&self) -> bool {
        self.waypoints.is_empty()
    }

    pub fn len(&self) -> usize {
        self.waypoints.len()
    }
}
