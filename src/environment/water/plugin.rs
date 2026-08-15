//! Water rendering plugin (ADR-053 E11).

use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

use super::ocean_material::EnvironmentOceanMaterial;
use super::settings::WaterSettings;
use super::spawn::{
    WaterSpawnState, ensure_environment_water, log_runtime_water_diagnostic_once,
    sync_environment_water_presentation,
};

/// Visual water surface presentation (Environment layer).
pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<EnvironmentOceanMaterial>::default())
            .register_type::<WaterSettings>()
            .init_resource::<WaterSettings>()
            .init_resource::<WaterSpawnState>()
            .add_systems(
                Update,
                (
                    ensure_environment_water,
                    sync_environment_water_presentation,
                    log_runtime_water_diagnostic_once,
                )
                    .chain(),
            );
    }
}
