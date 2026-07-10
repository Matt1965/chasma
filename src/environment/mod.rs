//! Environment rendering layer (R8 / ADR-026).
//!
//! Owns client-local skybox and global lighting presentation. Not part of
//! [`crate::world::WorldData`], terrain, biomes, or simulation.
//!
//! Future weather, atmosphere, water, and day/night systems extend this layer by
//! modifying [`EnvironmentSettings`] only.

mod cycle;
mod debug;
mod lighting;
mod singleton;
mod plugin;
mod settings;
mod skybox;
mod time_of_day;
mod water;

pub use cycle::{
    advance_time_of_day, apply_time_of_day_to_settings, daylight_factor, evaluate_time_of_day_lighting,
    sync_environment_presentation, twilight_warmth, update_environment_from_time_of_day,
    TimeOfDayLighting,
};
#[cfg(feature = "dev")]
pub use cycle::{
    apply_time_of_day_dev_action, format_time_of_day_status, time_of_day_dev_keyboard,
    TimeOfDayDevAction,
};

pub use debug::{
    count_environment_singletons, log_environment_configuration,
    log_environment_singleton_report, validate_environment_singletons, EnvironmentSingletonReport,
};
pub use singleton::{
    resolve_environment_directional_light, update_environment_directional_light,
    EnvironmentDirectionalLightResolution,
};
pub use plugin::EnvironmentPlugin;
pub use settings::{
    EnvironmentSettings, DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT, DEFAULT_DIRECTIONAL_LIGHT_POSITION,
    DEFAULT_SKYBOX_SET, ENVIRONMENT_ASSET_ROOT, SKYBOX_ASSET_ROOT,
};
pub use time_of_day::TimeOfDaySettings;
pub use water::{
    build_water_material, ensure_environment_water, sync_environment_water_presentation,
    water_plane_layout, EnvironmentWaterPlane, WaterPlaneLayout, WaterPlugin, WaterSettings,
    WaterSpawnState, DEFAULT_WATER_PLANE_SIZE_METERS,
};
pub use skybox::{
    cubemap_paths_for_set, disk_asset_path, loose_faces_exist, merge_loose_faces,
    merged_cubemap_path, resolve_existing_cubemap, skybox_set_dir, ActiveSkyboxLoad,
    SkyboxCamera, SkyboxCubemapPaths, SkyboxLoadStatus, CUBEMAP_KTX2_FILE, CUBEMAP_PNG_FILE,
    FACE_FILES_STACK_ORDER,
};
