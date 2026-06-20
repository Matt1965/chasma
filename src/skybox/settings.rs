use bevy::prelude::*;

/// Root folder for skybox cubemap assets (under `assets/`).
pub const SKYBOX_ASSET_ROOT: &str = "skyboxes";

/// Default skybox set loaded by dev preview (R8).
pub const DEFAULT_SKYBOX_SET: &str = "default";

/// Renderer-facing skybox configuration (R8).
///
/// Future day/night, weather, and biome systems swap [`Self::active_set`] or
/// replace the active cubemap through this resource — not through [`crate::world::WorldData`].
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct SkyboxSettings {
    /// Subfolder name under [`SKYBOX_ASSET_ROOT`] (e.g. `"default"`).
    pub active_set: String,
    /// Multiplier applied to cubemap samples (cd/m² after scaling).
    pub brightness: f32,
    /// View-space rotation applied to the cubemap.
    pub rotation: Quat,
}

impl Default for SkyboxSettings {
    fn default() -> Self {
        Self {
            active_set: DEFAULT_SKYBOX_SET.to_string(),
            brightness: 1_000.0,
            rotation: Quat::IDENTITY,
        }
    }
}
