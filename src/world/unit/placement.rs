use bevy::prelude::*;

use crate::world::WorldPosition;

/// Authoritative pose of a unit in world space (ADR-027 U2).
///
/// Position uses chunk-relative [`WorldPosition`], not a global [`Vec3`].
/// Rotation is stored directly; mesh and scale come from catalog definitions.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct UnitPlacement {
    pub position: WorldPosition,
    pub rotation: Quat,
}

impl UnitPlacement {
    pub fn new(position: WorldPosition, rotation: Quat) -> Self {
        Self { position, rotation }
    }
}
