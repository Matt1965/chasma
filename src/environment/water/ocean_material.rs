//! Depth-absorbing environment ocean material (WATER-DEPTH-1).

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::settings::WaterSettings;

/// Code-level tuning for screen-space depth absorption (no dev UI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterDepthPresentation {
    /// Alpha at zero water column depth (shallow / shoreline).
    pub shallow_alpha: f32,
    /// Alpha when depth response saturates (deep / open water).
    pub deep_alpha: f32,
    /// Exponential absorption scale in view-space meters (`1 - exp(-d / scale)`).
    pub absorption_depth: f32,
}

impl Default for WaterDepthPresentation {
    fn default() -> Self {
        Self {
            shallow_alpha: 0.38,
            deep_alpha: 0.90,
            absorption_depth: 12.0,
        }
    }
}

/// GPU uniform for [`EnvironmentOceanMaterial`].
#[derive(Debug, Clone, Copy, ShaderType)]
pub struct OceanDepthUniform {
    pub shallow_color: Vec4,
    pub deep_color: Vec4,
    pub absorption_depth: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

/// Custom transparent ocean material driven by prepass scene depth.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct EnvironmentOceanMaterial {
    #[uniform(0)]
    pub params: OceanDepthUniform,
}

impl Material for EnvironmentOceanMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/environment_ocean.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }
}

/// Smooth monotonic depth response shared with the WGSL shader.
pub fn evaluate_depth_response_factor(column_depth: f32, absorption_depth: f32) -> f32 {
    let scale = absorption_depth.max(0.001);
    (1.0 - (-column_depth.max(0.0) / scale).exp()).clamp(0.0, 1.0)
}

pub fn deep_ocean_color_from_shallow(shallow: Color) -> Color {
    let [r, g, b, _] = shallow.to_srgba().to_f32_array();
    Color::srgb(
        (r * 0.42).clamp(0.0, 1.0),
        (g * 0.72).clamp(0.0, 1.0),
        (b * 0.88).clamp(0.0, 1.0),
    )
}

pub fn build_ocean_depth_uniform(
    settings: &WaterSettings,
    presentation: WaterDepthPresentation,
) -> OceanDepthUniform {
    let [sr, sg, sb, _] = settings.color.to_srgba().to_f32_array();
    let [dr, dg, db, _] = deep_ocean_color_from_shallow(settings.color)
        .to_srgba()
        .to_f32_array();

    OceanDepthUniform {
        shallow_color: Vec4::new(sr, sg, sb, presentation.shallow_alpha),
        deep_color: Vec4::new(dr, dg, db, presentation.deep_alpha),
        absorption_depth: presentation.absorption_depth,
        _pad0: 0.0,
        _pad1: 0.0,
        _pad2: 0.0,
    }
}

pub fn build_ocean_material(settings: &WaterSettings) -> EnvironmentOceanMaterial {
    EnvironmentOceanMaterial {
        params: build_ocean_depth_uniform(settings, WaterDepthPresentation::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_presentation_defaults_are_ordered() {
        let presentation = WaterDepthPresentation::default();
        assert!(presentation.shallow_alpha < presentation.deep_alpha);
        assert!(presentation.shallow_alpha > 0.0);
        assert!(presentation.deep_alpha <= 1.0);
        assert!(presentation.absorption_depth > 0.0);
    }

    #[test]
    fn depth_response_is_monotonic_and_saturating() {
        let scale = WaterDepthPresentation::default().absorption_depth;
        let shallow = evaluate_depth_response_factor(0.0, scale);
        let mid = evaluate_depth_response_factor(scale, scale);
        let deep = evaluate_depth_response_factor(scale * 8.0, scale);
        assert!(shallow < mid);
        assert!(mid < deep);
        assert!(deep <= 1.0);
        assert!((deep - 1.0).abs() < 0.01);
    }

    #[test]
    fn ocean_material_uses_blend_mode() {
        let material = build_ocean_material(&WaterSettings::default());
        assert_eq!(material.alpha_mode(), AlphaMode::Blend);
        assert!(!EnvironmentOceanMaterial::enable_prepass());
    }

    #[test]
    fn shallow_color_follows_water_settings() {
        let mut settings = WaterSettings::default();
        settings.color = Color::srgb(0.1, 0.4, 0.6);
        settings.alpha = 0.55;
        let uniform = build_ocean_depth_uniform(&settings, WaterDepthPresentation::default());
        assert!((uniform.shallow_color.x - 0.1).abs() < 0.01);
        assert!((uniform.shallow_color.y - 0.4).abs() < 0.01);
        assert!((uniform.shallow_color.z - 0.6).abs() < 0.01);
        assert_eq!(
            uniform.shallow_color.w,
            WaterDepthPresentation::default().shallow_alpha
        );
        let shallow_luma = uniform.shallow_color.x * 0.2126
            + uniform.shallow_color.y * 0.7152
            + uniform.shallow_color.z * 0.0722;
        let deep_luma = uniform.deep_color.x * 0.2126
            + uniform.deep_color.y * 0.7152
            + uniform.deep_color.z * 0.0722;
        assert!(deep_luma < shallow_luma);
    }
}
