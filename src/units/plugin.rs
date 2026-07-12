use bevy::prelude::*;

use crate::world::UnitCatalog;

use super::animation::UnitAnimationPlugin;
use super::assets::preload_unit_scenes;
use super::components::{
    UnitRenderEntity, UnitRenderMetadata, UnitSceneRoot, UnitSelectionIndicator,
};
use super::settings::UnitsRuntimeSettings;
use super::spawn::UnitRenderIndex;
use super::sync::{UnitRuntimeSystems, sync_unit_render_entities};

/// Owns the Unit Runtime Layer (ADR-028).
pub struct UnitsRuntimePlugin;

impl Plugin for UnitsRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<UnitsRuntimeSettings>()
            .register_type::<UnitRenderEntity>()
            .register_type::<UnitRenderMetadata>()
            .register_type::<UnitSceneRoot>()
            .register_type::<UnitSelectionIndicator>()
            .init_resource::<UnitsRuntimeSettings>()
            .init_resource::<UnitRenderIndex>();

        #[cfg(feature = "dev")]
        app.init_resource::<crate::units::dev_spawn::DevPreviewUnitSpawnLedger>();

        app.add_plugins(UnitAnimationPlugin)
            .add_systems(Startup, init_unit_scene_assets)
            .add_systems(
                Update,
                (
                    #[cfg(feature = "dev")]
                    crate::units::dev_spawn::spawn_dev_preview_units,
                    sync_unit_render_entities,
                )
                    .chain()
                    .in_set(UnitRuntimeSystems),
            );
    }
}

fn init_unit_scene_assets(
    catalog: Res<UnitCatalog>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.insert_resource(preload_unit_scenes(&catalog, &asset_server));
}
