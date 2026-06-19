use bevy::prelude::*;

use crate::world::WorldPosition;

/// Authoritative pose of a doodad in world space (ADR-001, ADR-015).
///
/// Position uses chunk-relative [`WorldPosition`], not a global [`Vec3`].
/// Rotation and scale are stored directly; no mesh or asset references.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct DoodadPlacement {
    pub position: WorldPosition,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl DoodadPlacement {
    pub fn new(position: WorldPosition, rotation: Quat, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }
}
