//! Visual water rendering (ADR-053 E11).
//!
//! Client-local water planes at configured levels. Not terrain or simulation truth.

mod material;
mod plugin;
mod settings;
mod spawn;

pub use material::build_water_material;
pub use plugin::WaterPlugin;
pub use settings::{
    DEFAULT_WATER_EXTENT_PADDING_METERS, DEFAULT_WATER_PLANE_SIZE_METERS, WaterSettings,
};
pub use spawn::{
    AuthoredTerrainMeters, EnvironmentWaterPlane, WaterPlaneLayout, WaterSpawnState,
    WaterWorldBounds, ensure_environment_water, log_runtime_water_diagnostic_once,
    rectangle_mesh_xy_size, sync_environment_water_presentation, water_plane_layout,
};
