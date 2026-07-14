use bevy::prelude::*;

/// World pile merge and placement settings (ADR-090 I4).
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Resource)]
pub struct ItemPileSettings {
    /// Horizontal merge radius in meters.
    pub merge_radius_meters: f32,
    /// Quantization for stable distance comparison (centimeters).
    pub distance_quantize_cm: i32,
}

impl Default for ItemPileSettings {
    fn default() -> Self {
        Self {
            merge_radius_meters: 2.0,
            distance_quantize_cm: 1,
        }
    }
}

impl ItemPileSettings {
    pub fn merge_radius_squared_cm(&self) -> i64 {
        let radius_cm = (self.merge_radius_meters * 100.0).round() as i64;
        radius_cm.saturating_mul(radius_cm)
    }
}
