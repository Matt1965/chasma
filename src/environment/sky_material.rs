//! Procedural sky material (SKY-1).

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::visual_state::{
    EnvironmentVisualState, SUN_DISC_HALF_ANGLE_RAD, SUN_DISC_SOFTNESS_RAD, SkyColorPalette,
};

/// GPU uniform for [`EnvironmentSkyMaterial`].
#[derive(Debug, Clone, Copy, ShaderType)]
pub struct SkyPresentationUniform {
    pub horizon_color: Vec4,
    pub zenith_color: Vec4,
    pub twilight_color: Vec4,
    pub twilight_strength: f32,
    pub sun_direction: Vec3,
    pub sun_disc_color: Vec4,
    pub sun_disc_intensity: f32,
    pub sun_cos_radius: f32,
    pub sun_cos_softness: f32,
    pub _pad0: f32,
}

/// Custom opaque sky material driven by [`EnvironmentVisualState`].
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct EnvironmentSkyMaterial {
    #[uniform(0)]
    pub params: SkyPresentationUniform,
}

impl Material for EnvironmentSkyMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/environment_sky.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/environment_sky.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
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
        descriptor.primitive.cull_mode = Some(Face::Front);
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}

pub fn build_sky_presentation_uniform(visual: &EnvironmentVisualState) -> SkyPresentationUniform {
    let palette = SkyColorPalette::default();
    let [hr, hg, hb, _] = visual.sky_horizon_color.to_srgba().to_f32_array();
    let [zr, zg, zb, _] = visual.sky_zenith_color.to_srgba().to_f32_array();
    let [tr, tg, tb, _] = palette.twilight_glow.to_srgba().to_f32_array();
    let [sr, sg, sb, _] = visual.sun_disc_color.to_srgba().to_f32_array();

    SkyPresentationUniform {
        horizon_color: Vec4::new(hr, hg, hb, 1.0),
        zenith_color: Vec4::new(zr, zg, zb, 1.0),
        twilight_color: Vec4::new(tr, tg, tb, 1.0),
        twilight_strength: visual.twilight_factor,
        sun_direction: visual.sun_direction_world,
        sun_disc_color: Vec4::new(sr, sg, sb, 1.0),
        sun_disc_intensity: visual.sun_disc_intensity,
        sun_cos_radius: (SUN_DISC_HALF_ANGLE_RAD * 2.0).cos(),
        sun_cos_softness: SUN_DISC_SOFTNESS_RAD,
        _pad0: 0.0,
    }
}

pub fn build_sky_material(visual: &EnvironmentVisualState) -> EnvironmentSkyMaterial {
    EnvironmentSkyMaterial {
        params: build_sky_presentation_uniform(visual),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{
        EnvironmentVisualState, TimeOfDaySettings, evaluate_environment_visual_state,
        twilight_localized_weight,
    };

    #[test]
    fn sky_material_disables_prepass_and_shadows() {
        assert!(!EnvironmentSkyMaterial::enable_prepass());
        assert!(!EnvironmentSkyMaterial::enable_shadows());
    }

    #[test]
    fn sky_uniform_uses_shared_sun_direction() {
        let visual = evaluate_environment_visual_state(
            &TimeOfDaySettings {
                time_hours: 10.0,
                ..Default::default()
            },
            &SkyColorPalette::default(),
        );
        let uniform = build_sky_presentation_uniform(&visual);
        assert!((uniform.sun_direction - visual.sun_direction_world).length() < f32::EPSILON);
    }

    #[test]
    fn zero_twilight_spatial_weight_preserves_base_horizon_at_zenith() {
        assert!(twilight_localized_weight(1.0, 1.0, 1.0) < f32::EPSILON);
    }
}
