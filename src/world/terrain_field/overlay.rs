use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::TerrainFieldDefinitionError;

/// Presentation data for TF3 overlays (stored in TF1, not rendered yet).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TerrainFieldOverlayStyle {
    pub enabled: bool,
    pub low_color: Color,
    pub mid_color: Option<Color>,
    pub high_color: Color,
    pub default_opacity: f32,
    /// Values below this `u16` threshold may fade toward transparent in TF3.
    pub visibility_cutoff: u16,
    pub qualitative_thresholds: Vec<u16>,
    pub qualitative_labels: Vec<String>,
    pub icon_key: Option<String>,
}

impl Default for TerrainFieldOverlayStyle {
    fn default() -> Self {
        Self {
            enabled: true,
            low_color: Color::srgba(0.1, 0.1, 0.5, 1.0),
            mid_color: None,
            high_color: Color::srgba(0.9, 0.9, 0.2, 1.0),
            default_opacity: 0.55,
            visibility_cutoff: 0,
            qualitative_thresholds: vec![16_384, 32_768, 49_152],
            qualitative_labels: vec![
                "Low".to_string(),
                "Moderate".to_string(),
                "High".to_string(),
            ],
            icon_key: None,
        }
    }
}

impl TerrainFieldOverlayStyle {
    pub fn validate(&self) -> Result<(), TerrainFieldDefinitionError> {
        if !self.default_opacity.is_finite() || !(0.0..=1.0).contains(&self.default_opacity) {
            return Err(TerrainFieldDefinitionError::InvalidOverlayOpacity);
        }
        let mut prev = None;
        for threshold in &self.qualitative_thresholds {
            if let Some(p) = prev {
                if *threshold <= p {
                    return Err(TerrainFieldDefinitionError::UnsortedQualitativeThresholds);
                }
            }
            prev = Some(*threshold);
        }
        if !self.qualitative_labels.is_empty()
            && self.qualitative_labels.len() != self.qualitative_thresholds.len()
        {
            return Err(TerrainFieldDefinitionError::QualitativeLabelCountMismatch);
        }
        Ok(())
    }

    /// Map a normalized field sample to an overlay vertex color (ADR-103).
    pub fn vertex_color_for_value(&self, value: u16, player_opacity_bp: u16) -> Color {
        use super::basis_points::BasisPoints;
        let player_alpha = (BasisPoints::new(player_opacity_bp.min(9_000)).value() as f32
            / super::basis_points::BASIS_POINTS_ONE_HUNDRED_PERCENT as f32)
            .clamp(0.0, 0.9);
        if value < self.visibility_cutoff {
            return Color::srgba(0.0, 0.0, 0.0, 0.0);
        }
        let t = value as f32 / 65_535.0;
        let rgb = gradient_rgb(self.low_color, self.mid_color, self.high_color, t);
        Color::srgba(rgb.x, rgb.y, rgb.z, player_alpha)
    }

    /// Distinct unknown/unavailable presentation (checker modulation via `phase`).
    pub fn unknown_vertex_color(&self, player_opacity_bp: u16, phase: bool) -> Color {
        use super::basis_points::BasisPoints;
        let base_alpha = (BasisPoints::new(player_opacity_bp.min(9_000)).value() as f32
            / super::basis_points::BASIS_POINTS_ONE_HUNDRED_PERCENT as f32
            * 0.45)
            .clamp(0.12, 0.5);
        if phase {
            Color::srgba(0.45, 0.28, 0.55, base_alpha)
        } else {
            Color::srgba(0.32, 0.32, 0.38, base_alpha * 0.85)
        }
    }

    pub fn qualitative_label_for_value(&self, value: u16) -> Option<&str> {
        if self.qualitative_labels.is_empty() {
            return None;
        }
        for (index, threshold) in self.qualitative_thresholds.iter().enumerate() {
            if value < *threshold {
                return self.qualitative_labels.get(index).map(String::as_str);
            }
        }
        self.qualitative_labels.last().map(String::as_str)
    }
}

fn gradient_rgb(low: Color, mid: Option<Color>, high: Color, t: f32) -> Vec3 {
    let low = low.to_srgba();
    let high = high.to_srgba();
    let t = t.clamp(0.0, 1.0);
    let rgb = if let Some(mid) = mid {
        let mid = mid.to_srgba();
        if t <= 0.5 {
            lerp3(
                Vec3::new(low.red, low.green, low.blue),
                Vec3::new(mid.red, mid.green, mid.blue),
                t * 2.0,
            )
        } else {
            lerp3(
                Vec3::new(mid.red, mid.green, mid.blue),
                Vec3::new(high.red, high.green, high.blue),
                (t - 0.5) * 2.0,
            )
        }
    } else {
        lerp3(
            Vec3::new(low.red, low.green, low.blue),
            Vec3::new(high.red, high.green, high.blue),
            t,
        )
    };
    rgb
}

fn lerp3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
