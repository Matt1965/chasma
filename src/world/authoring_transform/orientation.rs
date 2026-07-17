//! Quantized Euler orientation for deterministic authoring (ADR-097 DT1).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Signed millidegrees per axis, canonical range `(-180_000, 180_000]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub struct QuantizedOrientation {
    pub yaw_millidegrees: i32,
    pub pitch_millidegrees: i32,
    pub roll_millidegrees: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientationError {
    NonFiniteDegrees,
    OutOfRange,
}

const MILLIDEGREES_PER_DEGREE: i32 = 1_000;
const HALF_TURN_MILLIDEGREES: i32 = 180_000;

impl QuantizedOrientation {
    pub const IDENTITY: Self = Self {
        yaw_millidegrees: 0,
        pitch_millidegrees: 0,
        roll_millidegrees: 0,
    };

    pub fn from_degrees(yaw: f32, pitch: f32, roll: f32) -> Result<Self, OrientationError> {
        if !yaw.is_finite() || !pitch.is_finite() || !roll.is_finite() {
            return Err(OrientationError::NonFiniteDegrees);
        }
        Ok(Self {
            yaw_millidegrees: degrees_to_millidegrees(yaw)?,
            pitch_millidegrees: degrees_to_millidegrees(pitch)?,
            roll_millidegrees: degrees_to_millidegrees(roll)?,
        })
    }

    pub fn from_millidegrees(
        yaw_millidegrees: i32,
        pitch_millidegrees: i32,
        roll_millidegrees: i32,
    ) -> Result<Self, OrientationError> {
        Ok(Self {
            yaw_millidegrees: canonicalize_millidegrees(yaw_millidegrees)?,
            pitch_millidegrees: canonicalize_millidegrees(pitch_millidegrees)?,
            roll_millidegrees: canonicalize_millidegrees(roll_millidegrees)?,
        })
    }

    pub fn yaw_degrees(self) -> f32 {
        self.yaw_millidegrees as f32 / MILLIDEGREES_PER_DEGREE as f32
    }

    pub fn pitch_degrees(self) -> f32 {
        self.pitch_millidegrees as f32 / MILLIDEGREES_PER_DEGREE as f32
    }

    pub fn roll_degrees(self) -> f32 {
        self.roll_millidegrees as f32 / MILLIDEGREES_PER_DEGREE as f32
    }

    /// Authoritative runtime conversion order: **YXZ** (yaw, pitch, roll).
    pub fn to_quat(self) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            self.yaw_degrees().to_radians(),
            self.pitch_degrees().to_radians(),
            self.roll_degrees().to_radians(),
        )
    }

    pub fn from_quat(quat: Quat) -> Result<Self, OrientationError> {
        let (yaw, pitch, roll) = quat.to_euler(EulerRot::YXZ);
        Self::from_degrees(yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees())
    }
}

pub fn degrees_to_millidegrees(degrees: f32) -> Result<i32, OrientationError> {
    if !degrees.is_finite() {
        return Err(OrientationError::NonFiniteDegrees);
    }
    let scaled = (degrees * MILLIDEGREES_PER_DEGREE as f32).round();
    if scaled < i32::MIN as f32 || scaled > i32::MAX as f32 {
        return Err(OrientationError::OutOfRange);
    }
    canonicalize_millidegrees(scaled as i32)
}

pub fn canonicalize_millidegrees(value: i32) -> Result<i32, OrientationError> {
    let mut v = value % (2 * HALF_TURN_MILLIDEGREES);
    if v <= -HALF_TURN_MILLIDEGREES {
        v += 2 * HALF_TURN_MILLIDEGREES;
    } else if v > HALF_TURN_MILLIDEGREES {
        v -= 2 * HALF_TURN_MILLIDEGREES;
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_millidegrees() {
        assert_eq!(canonicalize_millidegrees(0).unwrap(), 0);
        assert_eq!(canonicalize_millidegrees(180_000).unwrap(), 180_000);
        // (-180_000, 180_000] canonical range maps -180° to +180°.
        assert_eq!(canonicalize_millidegrees(-180_000).unwrap(), 180_000);
        assert_eq!(canonicalize_millidegrees(181_000).unwrap(), -179_000);
    }

    #[test]
    fn degree_round_trip() {
        let orientation = QuantizedOrientation::from_degrees(45.0, -12.5, 90.0).unwrap();
        assert!((orientation.yaw_degrees() - 45.0).abs() < 0.001);
        assert!((orientation.pitch_degrees() + 12.5).abs() < 0.001);
        assert!((orientation.roll_degrees() - 90.0).abs() < 0.001);
    }

    #[test]
    fn quaternion_round_trip() {
        let original = QuantizedOrientation::from_degrees(30.0, 15.0, -45.0).unwrap();
        let quat = original.to_quat();
        let restored = QuantizedOrientation::from_quat(quat).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn rejects_non_finite() {
        assert!(QuantizedOrientation::from_degrees(f32::NAN, 0.0, 0.0).is_err());
    }
}
