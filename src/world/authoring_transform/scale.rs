//! Quantized scale values for deterministic authoring (ADR-097 DT1).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Scale in milliunits where `1000` = `1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub struct FixedScale(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleError {
    OutOfRange,
    NonFinite,
    NonPositive,
    Zero,
}

pub const SCALE_MILLI_ONE: i32 = 1_000;
pub const SCALE_MILLI_MIN: i32 = 50;
pub const SCALE_MILLI_MAX: i32 = 20_000;

impl FixedScale {
    pub const ONE: Self = Self(SCALE_MILLI_ONE);

    pub fn from_milli(value: i32) -> Result<Self, ScaleError> {
        if value < SCALE_MILLI_MIN || value > SCALE_MILLI_MAX {
            return Err(ScaleError::OutOfRange);
        }
        Ok(Self(value))
    }

    pub fn from_f32(value: f32) -> Result<Self, ScaleError> {
        if !value.is_finite() {
            return Err(ScaleError::NonFinite);
        }
        if value <= 0.0 {
            return Err(if value == 0.0 {
                ScaleError::Zero
            } else {
                ScaleError::NonPositive
            });
        }
        let milli = (value * SCALE_MILLI_ONE as f32).round() as i32;
        Self::from_milli(milli)
    }

    pub fn milli(self) -> i32 {
        self.0
    }

    pub fn to_f32(self) -> f32 {
        self.0 as f32 / SCALE_MILLI_ONE as f32
    }

    pub fn to_vec3(self) -> Vec3 {
        Vec3::splat(self.to_f32())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum AuthoringScale {
    Uniform(FixedScale),
    NonUniform {
        x: FixedScale,
        y: FixedScale,
        z: FixedScale,
    },
}

impl AuthoringScale {
    pub fn uniform_one() -> Self {
        Self::Uniform(FixedScale::ONE)
    }

    pub fn from_uniform_f32(value: f32) -> Result<Self, ScaleError> {
        Ok(Self::Uniform(FixedScale::from_f32(value)?))
    }

    pub fn from_non_uniform_f32(x: f32, y: f32, z: f32) -> Result<Self, ScaleError> {
        Ok(Self::NonUniform {
            x: FixedScale::from_f32(x)?,
            y: FixedScale::from_f32(y)?,
            z: FixedScale::from_f32(z)?,
        })
    }

    pub fn to_vec3(self) -> Vec3 {
        match self {
            Self::Uniform(scale) => scale.to_vec3(),
            Self::NonUniform { x, y, z } => Vec3::new(x.to_f32(), y.to_f32(), z.to_f32()),
        }
    }

    pub fn uniform_value(self) -> Result<f32, ScaleError> {
        match self {
            Self::Uniform(scale) => Ok(scale.to_f32()),
            Self::NonUniform { .. } => Err(ScaleError::OutOfRange),
        }
    }

    pub fn multiply_vec3(self, other: Vec3) -> Vec3 {
        self.to_vec3() * other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_scale_boundaries() {
        assert!(FixedScale::from_f32(0.05).is_ok());
        assert!(FixedScale::from_f32(20.0).is_ok());
        assert!(FixedScale::from_f32(0.04).is_err());
        assert!(FixedScale::from_f32(20.01).is_err());
        assert!(FixedScale::from_f32(0.0).is_err());
        assert!(FixedScale::from_f32(-1.0).is_err());
    }

    #[test]
    fn deterministic_quantization() {
        let a = FixedScale::from_f32(1.234).unwrap();
        let b = FixedScale::from_f32(1.234).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.milli(), 1_234);
    }

    #[test]
    fn authoring_scale_vec3() {
        let scale = AuthoringScale::from_non_uniform_f32(1.0, 2.0, 3.0).unwrap();
        assert_eq!(scale.to_vec3(), Vec3::new(1.0, 2.0, 3.0));
    }
}
