//! Water presentation settings (ADR-053 E11).

use bevy::prelude::*;

/// Default square plane size when authored world extent is not yet available.
pub const DEFAULT_WATER_PLANE_SIZE_METERS: f32 = 2048.0;

/// Visual-only water configuration (Environment layer; not simulation truth).
#[derive(Debug, Clone, Resource, Reflect, PartialEq)]
#[reflect(Resource)]
pub struct WaterSettings {
    pub enabled: bool,
    /// World-space Y height of the water surface.
    pub water_level: f32,
    /// Fallback plane edge length when [`crate::world::WorldData::extent`] is unset.
    pub plane_size_meters: f32,
    pub color: Color,
    pub alpha: f32,
    pub roughness: f32,
    pub metallic: f32,
    /// Reserved for future shader animation (E11 does not animate).
    pub wave_speed: f32,
    /// Reserved for future shader animation (E11 does not animate).
    pub wave_scale: f32,
}

impl Default for WaterSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            water_level: 56.0,
            plane_size_meters: DEFAULT_WATER_PLANE_SIZE_METERS,
            color: Color::srgb(0.08, 0.32, 0.52),
            alpha: 0.62,
            roughness: 0.08,
            metallic: 0.15,
            wave_speed: 0.4,
            wave_scale: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_settings_defaults_are_sane() {
        let settings = WaterSettings::default();
        assert!(settings.enabled);
        assert!(settings.water_level.is_finite());
        assert!(settings.plane_size_meters > 0.0);
        assert!(settings.alpha > 0.0 && settings.alpha <= 1.0);
        assert!(settings.roughness >= 0.0);
        assert!(settings.metallic >= 0.0);
    }
}
