//! Procedural volumetric cloud material (CLOUD-1F, CLOUD-VOL-1, CLOUD-VOL-V2A).

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Face, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::cloud_settings::{CloudLayerId, CloudLayerSettings, CloudSettings, layer_night_factor};
use super::visual_state::EnvironmentVisualState;

/// GPU uniform for [`EnvironmentCloudMaterial`].
#[derive(Debug, Clone, Copy, ShaderType)]
pub struct CloudLayerUniform {
    pub wind_offset: Vec2,
    pub wind_direction: Vec2,
    pub coverage: f32,
    pub macro_scale: f32,
    pub vertical_development: f32,
    pub density_scale: f32,
    pub edge_breakup: f32,
    pub anisotropy: f32,
    pub layer_night_factor: f32,
    pub effective_daylight: f32,
    pub twilight_factor: f32,
    pub sun_direction: Vec3,
    pub _pad0: f32,
    pub sun_color: Vec4,
    pub low_y_min: f32,
    pub low_y_max: f32,
}

/// Custom transparent volumetric cloud material driven by [`EnvironmentVisualState`].
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct EnvironmentCloudMaterial {
    #[uniform(0)]
    pub params: CloudLayerUniform,
}

impl Material for EnvironmentCloudMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/environment_cloud.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/environment_cloud.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Proxy dome viewed from inside; cull outward-facing front faces.
        descriptor.primitive.cull_mode = Some(Face::Front);
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            // Proxy depth is raster coverage only; terrain ordering is decided in shader via prepass.
            depth.depth_write_enabled = false;
            depth.depth_compare = CompareFunction::Always;
        }
        Ok(())
    }
}

pub fn build_cloud_layer_uniform(
    visual: &EnvironmentVisualState,
    layer_settings: &CloudLayerSettings,
    settings: &CloudSettings,
    wind_offset: Vec2,
) -> CloudLayerUniform {
    let [sr, sg, sb, _] = visual.sun_color.to_srgba().to_f32_array();
    CloudLayerUniform {
        wind_offset,
        wind_direction: layer_settings.wind_direction.normalize_or_zero(),
        coverage: layer_settings.coverage,
        macro_scale: layer_settings.macro_scale,
        vertical_development: layer_settings.vertical_development,
        density_scale: layer_settings.density_scale,
        edge_breakup: layer_settings.edge_breakup,
        anisotropy: layer_settings.anisotropy,
        layer_night_factor: layer_night_factor(visual.night_factor, CloudLayerId::Low, settings),
        effective_daylight: visual.effective_daylight,
        twilight_factor: visual.twilight_factor,
        sun_direction: visual.sun_direction_world,
        _pad0: 0.0,
        sun_color: Vec4::new(sr, sg, sb, 1.0),
        low_y_min: settings.low_band.y_min,
        low_y_max: settings.low_band.y_max,
    }
}

pub fn build_cloud_material(
    visual: &EnvironmentVisualState,
    settings: &CloudSettings,
    wind_offset: Vec2,
) -> EnvironmentCloudMaterial {
    EnvironmentCloudMaterial {
        params: build_cloud_layer_uniform(visual, &settings.low, settings, wind_offset),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{
        SkyColorPalette, TimeOfDaySettings, evaluate_environment_visual_state,
    };

    #[test]
    fn cloud_material_disables_prepass_and_shadows() {
        assert!(!EnvironmentCloudMaterial::enable_prepass());
        assert!(!EnvironmentCloudMaterial::enable_shadows());
    }

    #[test]
    fn cloud_uniform_uses_shared_environment_visual_state() {
        let visual = evaluate_environment_visual_state(
            &TimeOfDaySettings {
                time_hours: 9.0,
                ..Default::default()
            },
            &SkyColorPalette::default(),
        );
        let settings = CloudSettings::default();
        let uniform =
            build_cloud_layer_uniform(&visual, &settings.low, &settings, Vec2::new(1.0, 2.0));
        assert!((uniform.sun_direction - visual.sun_direction_world).length() < f32::EPSILON);
        assert_eq!(uniform.effective_daylight, visual.effective_daylight);
        assert_eq!(uniform.twilight_factor, visual.twilight_factor);
        assert_eq!(uniform.wind_offset, Vec2::new(1.0, 2.0));
        assert_eq!(uniform.macro_scale, settings.low.macro_scale);
        assert_eq!(uniform.coverage, settings.low.coverage);
        assert_eq!(uniform.density_scale, settings.low.density_scale);
        assert_eq!(uniform.edge_breakup, settings.low.edge_breakup);
        assert_eq!(
            uniform.vertical_development,
            settings.low.vertical_development
        );
        assert_eq!(uniform.low_y_min, settings.low_band.y_min);
    }
}
