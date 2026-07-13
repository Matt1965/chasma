use bevy::prelude::*;

use crate::world::WorldPosition;

/// Authoritative anchor pose of a building in world space (ADR-079 B2).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct BuildingPlacement {
    pub position: WorldPosition,
    pub rotation: Quat,
}

impl BuildingPlacement {
    pub fn new(position: WorldPosition, rotation: Quat) -> Self {
        Self { position, rotation }
    }
}
