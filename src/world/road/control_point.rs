use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::RoadError;

/// How a control point participates in spline corner behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum CornerMode {
    #[default]
    Auto,
    Sharp,
}

/// One authored spline control point in simulation XZ.
///
/// Y is intentionally omitted. Elevation is derived later from terrain baking.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Serialize, Deserialize)]
pub struct RoadControlPoint {
    pub x: f32,
    pub z: f32,
    pub corner_mode: CornerMode,
}

impl RoadControlPoint {
    pub fn new(x: f32, z: f32) -> Self {
        Self {
            x,
            z,
            corner_mode: CornerMode::default(),
        }
    }

    pub fn xz(&self) -> Vec2 {
        Vec2::new(self.x, self.z)
    }

    pub fn validate(&self) -> Result<(), RoadError> {
        if !self.x.is_finite() || !self.z.is_finite() {
            return Err(RoadError::InvalidControlPoint(
                "control point coordinates must be finite".to_string(),
            ));
        }
        Ok(())
    }
}
