use bevy::prelude::*;

use crate::world::WorldPosition;
use crate::world::authoring_transform::{
    AuthoringScale, FixedScale, OrientationError, QuantizedOrientation, ScaleError,
};

/// Authoritative pose of a doodad in world space (ADR-001, ADR-015, ADR-098 DT2).
///
/// Position uses chunk-relative [`WorldPosition`]. Orientation and scale use the
/// DT1 quantized authoring contract — not raw ECS [`Transform`] values.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct DoodadPlacement {
    pub position: WorldPosition,
    pub orientation: QuantizedOrientation,
    pub scale: AuthoringScale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoodadPlacementError {
    InvalidOrientation(OrientationError),
    InvalidScale(ScaleError),
}

impl DoodadPlacement {
    pub fn new(
        position: WorldPosition,
        orientation: QuantizedOrientation,
        scale: AuthoringScale,
    ) -> Self {
        Self {
            position,
            orientation,
            scale,
        }
    }

    pub fn identity_at(position: WorldPosition) -> Self {
        Self {
            position,
            orientation: QuantizedOrientation::IDENTITY,
            scale: AuthoringScale::uniform_one(),
        }
    }

    /// Construct from legacy Quat/Vec3 (import, scene migration).
    pub fn from_legacy(
        position: WorldPosition,
        rotation: Quat,
        scale: Vec3,
    ) -> Result<Self, DoodadPlacementError> {
        let orientation = QuantizedOrientation::from_quat(rotation)
            .map_err(DoodadPlacementError::InvalidOrientation)?;
        let scale = AuthoringScale::from_non_uniform_f32(scale.x, scale.y, scale.z)
            .map_err(DoodadPlacementError::InvalidScale)?;
        Ok(Self::new(position, orientation, scale))
    }

    pub fn from_millidegrees_and_scale(
        position: WorldPosition,
        yaw_mdeg: i32,
        pitch_mdeg: i32,
        roll_mdeg: i32,
        scale_x_milli: i32,
        scale_y_milli: i32,
        scale_z_milli: i32,
    ) -> Result<Self, DoodadPlacementError> {
        let orientation = QuantizedOrientation::from_millidegrees(yaw_mdeg, pitch_mdeg, roll_mdeg)
            .map_err(DoodadPlacementError::InvalidOrientation)?;
        let scale = AuthoringScale::NonUniform {
            x: FixedScale::from_milli(scale_x_milli).map_err(DoodadPlacementError::InvalidScale)?,
            y: FixedScale::from_milli(scale_y_milli).map_err(DoodadPlacementError::InvalidScale)?,
            z: FixedScale::from_milli(scale_z_milli).map_err(DoodadPlacementError::InvalidScale)?,
        };
        Ok(Self::new(position, orientation, scale))
    }

    pub fn rotation_quat(self) -> Quat {
        self.orientation.to_quat()
    }

    pub fn scale_vec3(self) -> Vec3 {
        self.scale.to_vec3()
    }

    pub fn yaw_radians(self) -> f32 {
        self.orientation.yaw_degrees().to_radians()
    }

    /// Ground collision uses yaw only (pitch/roll are visual).
    pub fn collision_yaw_radians(self) -> f32 {
        self.yaw_radians()
    }

    /// Horizontal collision scale from instance X/Z (Y is visual-only for ground collision).
    pub fn collision_scale_xz(self) -> Vec2 {
        let s = self.scale_vec3();
        Vec2::new(s.x, s.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};

    #[test]
    fn legacy_round_trip() {
        let pos = WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ONE));
        let placement =
            DoodadPlacement::from_legacy(pos, Quat::from_rotation_y(0.5), Vec3::splat(1.1))
                .unwrap();
        assert!((placement.scale_vec3().x - 1.1).abs() < 0.02);
        assert!((placement.yaw_radians() - 0.5).abs() < 0.05);
    }
}
