//! Client-local snap settings for transform gizmos (ADR-099).

use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct TransformSnapSettings {
    pub translation_enabled: bool,
    pub translation_step_meters: f32,
    pub rotation_enabled: bool,
    pub rotation_step_degrees: f32,
    pub scale_enabled: bool,
    pub scale_step: f32,
}

impl Default for TransformSnapSettings {
    fn default() -> Self {
        Self {
            translation_enabled: false,
            translation_step_meters: 0.1,
            rotation_enabled: false,
            rotation_step_degrees: 5.0,
            scale_enabled: false,
            scale_step: 0.05,
        }
    }
}

impl TransformSnapSettings {
    pub fn snap_translation(self, value: f32, finer: bool) -> f32 {
        if !self.translation_enabled {
            return value;
        }
        let step = if finer {
            self.translation_step_meters * 0.1
        } else {
            self.translation_step_meters
        };
        (value / step).round() * step
    }

    pub fn snap_rotation_degrees(self, degrees: f32, finer: bool) -> f32 {
        if !self.rotation_enabled {
            return degrees;
        }
        let step = if finer {
            self.rotation_step_degrees * 0.2
        } else {
            self.rotation_step_degrees
        };
        (degrees / step).round() * step
    }

    pub fn snap_scale(self, value: f32, finer: bool) -> f32 {
        if !self.scale_enabled {
            return value;
        }
        let step = if finer {
            self.scale_step * 0.2
        } else {
            self.scale_step
        };
        (value / step).round() * step
    }

    pub fn toggle_translation(&mut self) {
        self.translation_enabled = !self.translation_enabled;
    }

    pub fn toggle_rotation(&mut self) {
        self.rotation_enabled = !self.rotation_enabled;
    }

    pub fn toggle_scale(&mut self) {
        self.scale_enabled = !self.scale_enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_snap_rounds() {
        let mut snap = TransformSnapSettings::default();
        snap.translation_enabled = true;
        snap.translation_step_meters = 0.5;
        assert!((snap.snap_translation(1.24, false) - 1.0).abs() < f32::EPSILON);
        assert!((snap.snap_translation(1.26, false) - 1.5).abs() < f32::EPSILON);
    }
}
