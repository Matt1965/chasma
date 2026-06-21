//! Environment rendering layer (R8 / ADR-026).
//!
//! Owns client-local skybox and global lighting presentation. Not part of
//! [`crate::world::WorldData`], terrain, biomes, or simulation.
//!
//! Future weather, atmosphere, water, and day/night systems extend this layer by
//! modifying [`EnvironmentSettings`] only.

mod debug;
mod lighting;
mod plugin;
mod settings;
mod skybox;

pub use debug::{
    count_environment_singletons, log_environment_configuration,
    log_environment_singleton_report, validate_environment_singletons, EnvironmentSingletonReport,
};
pub use lighting::{EnvironmentDirectionalLight, EnvironmentLightingInitialized};
pub use plugin::EnvironmentPlugin;
pub use settings::{
    EnvironmentSettings, DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT, DEFAULT_DIRECTIONAL_LIGHT_POSITION,
    DEFAULT_SKYBOX_SET, ENVIRONMENT_ASSET_ROOT, SKYBOX_ASSET_ROOT,
};
pub use skybox::{
    cubemap_paths_for_set, disk_asset_path, loose_faces_exist, merge_loose_faces,
    merged_cubemap_path, resolve_existing_cubemap, skybox_set_dir, ActiveSkyboxLoad,
    SkyboxCamera, SkyboxCubemapPaths, SkyboxLoadStatus, CUBEMAP_KTX2_FILE, CUBEMAP_PNG_FILE,
    FACE_FILES_STACK_ORDER,
};
