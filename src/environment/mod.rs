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
mod plugin;
mod settings;
mod singleton;
mod skybox;
mod time_of_day;
mod water;

#[cfg(feature = "dev")]
pub use cycle::{
    TimeOfDayDevAction, apply_time_of_day_dev_action, format_time_of_day_status,
    time_of_day_dev_keyboard,
};
pub use cycle::{
    TimeOfDayLighting, advance_time_of_day, apply_time_of_day_to_settings, daylight_factor,
    evaluate_time_of_day_lighting, sync_environment_presentation, twilight_warmth,
    update_environment_from_time_of_day,
};

pub use debug::{
    EnvironmentSingletonReport, count_environment_singletons, log_environment_configuration,
    log_environment_singleton_report, validate_environment_singletons,
};
pub use plugin::EnvironmentPlugin;
pub use settings::{
    DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT, DEFAULT_DIRECTIONAL_LIGHT_POSITION, DEFAULT_SKYBOX_SET,
    ENVIRONMENT_ASSET_ROOT, EnvironmentSettings, SKYBOX_ASSET_ROOT,
};
pub use singleton::{
    EnvironmentDirectionalLightResolution, resolve_environment_directional_light,
    update_environment_directional_light,
};
pub use skybox::{
    ActiveSkyboxLoad, CUBEMAP_KTX2_FILE, CUBEMAP_PNG_FILE, FACE_FILES_STACK_ORDER, SkyboxCamera,
    SkyboxCubemapPaths, SkyboxLoadStatus, cubemap_paths_for_set, disk_asset_path,
    loose_faces_exist, merge_loose_faces, merged_cubemap_path, resolve_existing_cubemap,
    skybox_set_dir,
};
pub use time_of_day::TimeOfDaySettings;
pub use water::{
    DEFAULT_WATER_PLANE_SIZE_METERS, EnvironmentWaterPlane, WaterPlaneLayout, WaterPlugin,
    WaterSettings, WaterSpawnState, build_water_material, ensure_environment_water,
    sync_environment_water_presentation, water_plane_layout,
};
