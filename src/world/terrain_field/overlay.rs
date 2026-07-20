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
        let t = self.display_gradient_t(value);
        let rgb = gradient_rgb(self.low_color, self.mid_color, self.high_color, t);
        // Dry areas stay faint; wet areas read stronger so placement decisions pop.
        let value_strength = (t.sqrt() * 0.88 + 0.08).clamp(0.08, 1.0);
        let alpha = player_alpha * value_strength;
        Color::srgba(rgb.x, rgb.y, rgb.z, alpha)
    }

    /// Stretch authored qualitative bands across the full dry→wet gradient.
    fn display_gradient_t(&self, value: u16) -> f32 {
        let thresholds = &self.qualitative_thresholds;
        if thresholds.len() < 2 {
            return value as f32 / 65_535.0;
        }
        let v = value as f32;
        let segment = 1.0 / thresholds.len() as f32;
        if v <= thresholds[0] as f32 {
            return (v / thresholds[0] as f32) * segment;
        }
        for index in 1..thresholds.len() {
            let prev = thresholds[index - 1] as f32;
            let next = thresholds[index] as f32;
            if v <= next {
                let local = (v - prev) / (next - prev);
                return index as f32 * segment + local * segment;
            }
        }
        1.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_gradient_stretches_low_values_across_dry_band() {
        let style = TerrainFieldOverlayStyle {
            qualitative_thresholds: vec![9_830, 26_214, 42_598],
            qualitative_labels: vec!["Dry".into(), "Moderate".into(), "Wet".into()],
            ..Default::default()
        };
        assert!(style.display_gradient_t(0) < 0.01);
        assert!(style.display_gradient_t(4_915) < 0.2);
        assert!((style.display_gradient_t(26_214) - 0.66).abs() < 0.05);
    }

    #[test]
    fn dry_values_still_render_with_faint_alpha() {
        let style = TerrainFieldOverlayStyle::default();
        let dry = style.vertex_color_for_value(2_000, 5_500);
        assert!(dry.alpha() > 0.05);
        assert!(dry.alpha() < 0.35);
    }
}
