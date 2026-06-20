//! Skybox rendering layer (R8 / ADR-026).
//!
//! Owns client-local cubemap presentation for the dev preview. Not part of
//! [`crate::world::WorldData`], terrain, biomes, or simulation.

use bevy::prelude::*;

mod assets;
mod load;
mod merge;
mod settings;

pub use assets::{
    cubemap_paths_for_set, disk_asset_path, resolve_existing_cubemap, CUBEMAP_KTX2_FILE,
    CUBEMAP_PNG_FILE, SkyboxCubemapPaths,
};
pub use load::{ActiveSkyboxLoad, SkyboxCamera};
pub use merge::{
    loose_faces_exist, merge_loose_faces, merged_cubemap_path, skybox_set_dir,
    FACE_FILES_STACK_ORDER,
};
pub use settings::{SkyboxSettings, DEFAULT_SKYBOX_SET, SKYBOX_ASSET_ROOT};

/// Loads and displays the active skybox on the primary 3D camera.
pub struct SkyboxPlugin;

impl Plugin for SkyboxPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SkyboxSettings>()
            .init_resource::<SkyboxSettings>()
            .add_systems(Startup, load::init_active_skybox_load)
            .add_systems(Update, load::attach_skybox_to_primary_camera);
    }
}
