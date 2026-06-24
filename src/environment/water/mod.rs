//! Visual water rendering (ADR-053 E11).
//!
//! Client-local water planes at configured levels. Not terrain or simulation truth.

mod material;
mod plugin;
mod settings;
mod spawn;

pub use material::build_water_material;
pub use plugin::WaterPlugin;
pub use settings::{WaterSettings, DEFAULT_WATER_PLANE_SIZE_METERS};
pub use spawn::{
    ensure_environment_water, sync_environment_water_presentation, water_plane_layout,
    EnvironmentWaterPlane, WaterPlaneLayout, WaterSpawnState,
};
