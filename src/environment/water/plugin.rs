//! Water rendering plugin (ADR-053 E11).

use bevy::prelude::*;

use super::settings::WaterSettings;
use super::spawn::{
    ensure_environment_water, sync_environment_water_presentation, WaterSpawnState,
};

/// Visual water surface presentation (Environment layer).
pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<WaterSettings>()
            .init_resource::<WaterSettings>()
            .init_resource::<WaterSpawnState>()
            .add_systems(
                Update,
                (
                    ensure_environment_water,
                    sync_environment_water_presentation,
                )
                    .chain(),
            );

        #[cfg(feature = "dev")]
        app.add_systems(Update, super::spawn::water_dev_keyboard);
    }
}
