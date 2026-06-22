use bevy::prelude::*;

use crate::world::UnitCatalog;

use super::assets::preload_unit_scenes;
use super::components::{UnitRenderEntity, UnitSceneRoot};
use super::settings::UnitsRuntimeSettings;
use super::spawn::UnitRenderIndex;
use super::sync::{sync_unit_render_entities, UnitRuntimeSystems};

/// Owns the Unit Runtime Layer (ADR-028).
pub struct UnitsRuntimePlugin;

impl Plugin for UnitsRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<UnitsRuntimeSettings>()
            .register_type::<UnitRenderEntity>()
            .register_type::<UnitSceneRoot>()
            .init_resource::<UnitsRuntimeSettings>()
            .init_resource::<UnitRenderIndex>()
            .add_systems(Startup, init_unit_scene_assets)
            .add_systems(
                Update,
                sync_unit_render_entities.in_set(UnitRuntimeSystems),
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
