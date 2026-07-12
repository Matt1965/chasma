use bevy::prelude::*;

/// Root folder for environment presentation assets (under `assets/`).
pub const ENVIRONMENT_ASSET_ROOT: &str = "environment";

/// Skybox cubemap sets live under `assets/environment/skyboxes/{set_name}/`.
pub const SKYBOX_ASSET_ROOT: &str = "environment/skyboxes";

/// Default skybox set name (folder under [`SKYBOX_ASSET_ROOT`]).
pub const DEFAULT_SKYBOX_SET: &str = "default";

/// Default directional-light position used to derive [`Self::directional_light_rotation`].
pub const DEFAULT_DIRECTIONAL_LIGHT_POSITION: Vec3 = Vec3::new(256.0, 200.0, 128.0);

/// Default directional-light look-at target (dev preview map center).
pub const DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT: Vec3 = Vec3::new(256.0, 0.0, 128.0);

/// Renderer-facing environment configuration (R8 / ADR-026, R9 tuning).
///
/// All environment presentation tuning lives here. Future weather, day/night,
/// atmosphere, and biome lighting modify this resource only — not
/// [`crate::world::WorldData`] or gameplay state.
#[derive(Debug, Clone, Resource, Reflect, PartialEq)]
#[reflect(Resource)]
pub struct EnvironmentSettings {
    /// Skybox set folder name (e.g. `"default"` → `environment/skyboxes/default/`).
    pub skybox_set: String,
    /// Cubemap sample brightness (cd/m² after scaling).
    pub skybox_brightness: f32,
    /// View-space rotation applied to the cubemap.
    pub skybox_rotation: Quat,
    /// Directional sun/moon illuminance (lux).
    pub directional_light_illuminance: f32,
    pub directional_light_color: Color,
    /// World-space rotation for the directional light.
    pub directional_light_rotation: Quat,
    /// Global ambient fill brightness.
    pub ambient_brightness: f32,
    pub ambient_color: Color,
    pub directional_shadows_enabled: bool,
    /// Shadow cascade tuning for RTS orbit distances (DV3).
    pub shadow_cascade_count: usize,
    pub shadow_cascade_minimum_distance: f32,
    pub shadow_cascade_maximum_distance: f32,
    pub shadow_cascade_first_far_bound: f32,
    pub shadow_cascade_overlap: f32,
    pub directional_shadow_normal_bias: f32,
}

impl Default for EnvironmentSettings {
    fn default() -> Self {
        let directional_light_rotation =
            Transform::from_translation(DEFAULT_DIRECTIONAL_LIGHT_POSITION)
                .looking_at(DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT, Vec3::Y)
                .rotation;

        Self {
            skybox_set: DEFAULT_SKYBOX_SET.to_string(),
            skybox_brightness: 1_078.0,
            skybox_rotation: Quat::IDENTITY,
            directional_light_illuminance: 21_189.0,
            directional_light_color: Color::srgb(1.0, 0.97, 0.92),
            directional_light_rotation,
            ambient_brightness: 349.0,
            ambient_color: Color::srgb(0.85, 0.88, 0.95),
            directional_shadows_enabled: true,
            shadow_cascade_count: 4,
            shadow_cascade_minimum_distance: 0.5,
            // Bevy default maximum_distance (150) is too small for RTS orbit (40–5000+).
            shadow_cascade_maximum_distance: 2_500.0,
            shadow_cascade_first_far_bound: 120.0,
            shadow_cascade_overlap: 0.2,
            directional_shadow_normal_bias: 2.0,
        }
    }
}

impl EnvironmentSettings {
    /// Asset-server path prefix for the active skybox set (`environment/skyboxes/{set}`).
    pub fn skybox_set_path(&self) -> String {
        format!("{}/{}", SKYBOX_ASSET_ROOT, self.skybox_set)
    }

    /// Human-readable startup report for dev logging (R9).
    pub fn format_debug_report(&self) -> String {
        let (yaw, pitch, roll) = self.directional_light_rotation.to_euler(EulerRot::YXZ);
        let yaw_deg = yaw.to_degrees();
        let pitch_deg = pitch.to_degrees();
        let roll_deg = roll.to_degrees();

        format!(
            "Environment Settings\n\
             \n\
             Directional Light:\n\
             - illuminance: {:.0}\n\
             - rotation (yaw/pitch/roll deg): ({:.1}, {:.1}, {:.1})\n\
             - shadows enabled: {}\n\
             - cascade max distance: {:.0}\n\
             - cascade first bound: {:.0}\n\
             \n\
             Ambient Light:\n\
             - brightness: {:.0}\n\
             \n\
             Skybox:\n\
             - active set: {}\n\
             - brightness: {:.0}",
            self.directional_light_illuminance,
            yaw_deg,
            pitch_deg,
            roll_deg,
            self.directional_shadows_enabled,
            self.shadow_cascade_maximum_distance,
            self.shadow_cascade_first_far_bound,
            self.ambient_brightness,
            self.skybox_set,
            self.skybox_brightness,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_skybox_set_is_default() {
        let settings = EnvironmentSettings::default();
        assert_eq!(settings.skybox_set, DEFAULT_SKYBOX_SET);
    }

    #[test]
    fn skybox_set_path_uses_environment_root() {
        let settings = EnvironmentSettings::default();
        assert_eq!(settings.skybox_set_path(), "environment/skyboxes/default");
    }

    #[test]
    fn defaults_enable_shadows_with_positive_illuminance() {
        let settings = EnvironmentSettings::default();
        assert!(settings.directional_shadows_enabled);
        assert!(settings.directional_light_illuminance > 0.0);
        assert!(settings.ambient_brightness > 0.0);
        assert!(settings.skybox_brightness > 0.0);
    }

    #[test]
    fn defaults_use_rts_scaled_shadow_cascades() {
        let settings = EnvironmentSettings::default();
        assert!(settings.shadow_cascade_maximum_distance > 500.0);
        assert!(settings.shadow_cascade_first_far_bound > 50.0);
    }

    #[test]
    fn debug_report_includes_key_sections() {
        let report = EnvironmentSettings::default().format_debug_report();
        assert!(report.contains("Environment Settings"));
        assert!(report.contains("Directional Light:"));
        assert!(report.contains("Ambient Light:"));
        assert!(report.contains("Skybox:"));
        assert!(report.contains("shadows enabled"));
    }

    #[test]
    fn default_light_rotation_uses_documented_look_at() {
        let settings = EnvironmentSettings::default();
        let expected = Transform::from_translation(DEFAULT_DIRECTIONAL_LIGHT_POSITION)
            .looking_at(DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT, Vec3::Y)
            .rotation;
        assert_eq!(settings.directional_light_rotation, expected);
    }
}
