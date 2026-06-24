//! Water [`StandardMaterial`] builder (ADR-053 E11).

use bevy::prelude::*;

use super::settings::WaterSettings;

/// Build a lit, semi-transparent water material from settings.
pub fn build_water_material(settings: &WaterSettings) -> StandardMaterial {
    let [r, g, b, _] = settings.color.to_srgba().to_f32_array();
    StandardMaterial {
        base_color: Color::srgba(r, g, b, settings.alpha),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: settings.roughness,
        metallic: settings.metallic,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_material_uses_blend_alpha_mode() {
        let material = build_water_material(&WaterSettings::default());
        assert_eq!(material.alpha_mode, AlphaMode::Blend);
        assert!(!material.unlit);
    }
}
