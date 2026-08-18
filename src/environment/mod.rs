//! Environment rendering layer (R8 / ADR-026).
//!
//! Owns client-local skybox and global lighting presentation. Not part of
//! [`crate::world::WorldData`], terrain, biomes, or simulation.
//!
//! Future weather, atmosphere, water, and day/night systems extend this layer by
//! modifying [`EnvironmentSettings`] only.

mod cloud_material;
mod cloud_settings;
mod cycle;
mod debug;
mod lighting;
mod plugin;
mod procedural_clouds;
mod procedural_sky;
mod project_defaults;
mod settings;
mod singleton;
mod sky_material;
mod skybox;
mod time_of_day;
mod visual_state;
mod water;

#[cfg(feature = "dev")]
pub use cycle::{TimeOfDayDevAction, apply_time_of_day_dev_action, format_time_of_day_status};
pub use cycle::{
    TimeOfDayLighting, advance_time_of_day, apply_time_of_day_to_settings, daylight_factor,
    evaluate_time_of_day_lighting, sync_environment_presentation, twilight_warmth,
    update_environment_from_time_of_day,
};

pub use cloud_material::{
    CloudLayerUniform, EnvironmentCloudMaterial, build_cloud_layer_uniform, build_cloud_material,
};
pub use cloud_settings::{
    CLOUD_EROSION_HEIGHT_BIAS, CLOUD_EROSION_NOISE_RATIO, CLOUD_MARCH_MAX_DIST_METERS,
    CLOUD_MARCH_MAX_SEGMENT_METERS, CLOUD_MARCH_MAX_STEP_METERS, CLOUD_MARCH_MAX_STEPS,
    CLOUD_MARCH_MAX_STEPS_CAP, CLOUD_MARCH_MIN_STEP_METERS, CLOUD_TRANSMITTANCE_CUTOFF,
    CloudAltitudeBand, CloudLayerId, CloudLayerSettings, CloudMarchStepPlan, CloudSettings,
    DEFAULT_CLOUD_DENSITY_SCALE, DEFAULT_CLOUD_EDGE_BREAKUP, DEFAULT_CLOUD_VERTICAL_DEVELOPMENT,
    DEFAULT_HIGH_CLOUD_MACRO_SCALE, DEFAULT_HIGH_CLOUD_SCALE, DEFAULT_LOW_CLOUD_MACRO_SCALE,
    DEFAULT_LOW_CLOUD_SCALE, DEFAULT_LOW_CLOUD_Y_MAX, DEFAULT_LOW_CLOUD_Y_MIN,
    apply_erosion_to_shape, band_entry_distance_upward, base_feature_wavelength_meters,
    cloud_march_step_count, cloud_march_step_len, cloud_march_step_plan, cloud_march_t_limit,
    cloud_sample_delta_from_world, cloud_wind_displacement_world, cloud_wind_offset,
    erosion_height_bias, height_profile, intersect_ray_y_band, layer_night_factor,
    ray_band_march_start, remap, world_ray_sample,
};
pub use debug::{
    EnvironmentSingletonReport, count_environment_singletons, log_environment_configuration,
    log_environment_singleton_report, validate_environment_singletons,
};
pub use plugin::EnvironmentPlugin;
pub use procedural_clouds::{
    CLOUD_RENDER_PROXY_RADIUS, EnvironmentProceduralCloud, EnvironmentProceduralCloudHigh,
    EnvironmentProceduralCloudLow, ProceduralCloudSpawnState, setup_procedural_clouds,
    sync_procedural_cloud_presentation,
};
pub use procedural_sky::{
    EnvironmentProceduralSky, ProceduralSkySpawnState, setup_procedural_sky,
    sync_procedural_sky_presentation,
};
pub use project_defaults::{
    AuthoredEnvironment, AuthoredEnvironmentSnapshot, AuthoredTimeOfDay, EnvironmentManualLighting,
    EnvironmentValidationError, ManualLightingDefaults, PROJECT_DEFAULTS_PATH,
    PROJECT_DEFAULTS_VERSION, ProjectDefaultsLoadStatus, ProjectDefaultsSaveError,
    ProjectEnvironmentBaseline, apply_manual_lighting, built_in_authored_snapshot,
    capture_current_authored_snapshot, environment_is_dirty, initialize_runtime_from_baseline,
    list_registered_skybox_sets, load_project_environment_baseline,
    save_project_environment_defaults, skybox_set_exists, validate_authored_snapshot,
};
pub use settings::{
    DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT, DEFAULT_DIRECTIONAL_LIGHT_POSITION, DEFAULT_SKYBOX_SET,
    ENVIRONMENT_ASSET_ROOT, EnvironmentSettings, SKYBOX_ASSET_ROOT,
};
pub use singleton::{
    EnvironmentDirectionalLightResolution, resolve_environment_directional_light,
    update_environment_directional_light,
};
pub use sky_material::{
    EnvironmentSkyMaterial, SkyPresentationUniform, build_sky_material,
    build_sky_presentation_uniform,
};
pub use skybox::{
    ActiveSkyboxLoad, CUBEMAP_KTX2_FILE, CUBEMAP_PNG_FILE, FACE_FILES_STACK_ORDER, SkyboxCamera,
    SkyboxCubemapPaths, SkyboxLoadStatus, cubemap_paths_for_set, disk_asset_path,
    loose_faces_exist, merge_loose_faces, merged_cubemap_path, resolve_existing_cubemap,
    skybox_set_dir,
};
pub use time_of_day::TimeOfDaySettings;
pub use visual_state::{
    DEFAULT_SKY_PRESENTATION, EnvironmentVisualState, SKY_GRADIENT_HORIZON_EXPONENT,
    SUN_DISC_HALF_ANGLE_RAD, SUN_DISC_SOFTNESS_RAD, SkyColorPalette, SkyPresentation,
    TWILIGHT_SUN_ALIGNMENT_EXPONENT, apply_visual_state_to_environment,
    evaluate_environment_visual_state, light_travel_direction_from_sun, sky_horizon_weight,
    sun_direction_world_from_light_rotation, twilight_localized_weight,
    update_environment_visual_state,
};
pub use water::{
    AuthoredTerrainMeters, DEFAULT_WATER_EXTENT_PADDING_METERS, DEFAULT_WATER_PLANE_SIZE_METERS,
    EnvironmentWaterPlane, WaterPlaneLayout, WaterPlugin, WaterSettings, WaterSpawnState,
    WaterWorldBounds, build_water_material, ensure_environment_water, rectangle_mesh_xy_size,
    sync_environment_water_presentation, water_plane_layout,
};
