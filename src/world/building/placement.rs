use bevy::prelude::*;

use crate::world::WorldPosition;
use crate::world::authoring_transform::FixedScale;

/// Authoritative anchor pose of a building in world space (ADR-079 B2, ADR-100 DT4).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct BuildingPlacement {
    pub position: WorldPosition,
    pub rotation: Quat,
    /// Dev-only instance uniform scale (player Build Mode uses definition baseline).
    pub uniform_scale: FixedScale,
}

impl BuildingPlacement {
    pub fn new(position: WorldPosition, rotation: Quat) -> Self {
        Self {
            position,
            rotation,
            uniform_scale: FixedScale::ONE,
        }
    }

    pub fn with_uniform_scale(mut self, uniform_scale: FixedScale) -> Self {
        self.uniform_scale = uniform_scale;
        self
    }

    pub fn uniform_scale_f32(self) -> f32 {
        self.uniform_scale.to_f32()
    }
}
