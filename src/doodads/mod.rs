//! Doodad Runtime Layer (ADR-023).
//!
//! Owns derived, disposable runtime concerns for doodads: glTF asset handles,
//! ECS render entities, and sync with terrain residency. Authoritative instance
//! data remains in [`crate::world::WorldData`].

use bevy::prelude::*;

pub mod assets;
pub mod components;
#[cfg(feature = "dev")]
pub mod picking;
#[cfg(feature = "dev")]
pub mod procgen;
pub mod settings;
pub mod spawn;
pub mod sync;

pub use assets::{DOODAD_ASSET_ROOT, DoodadSceneAssets, gltf_asset_path, preload_doodad_scenes};
pub use components::DoodadRenderEntity;
pub use settings::{DEFAULT_DOODAD_WORLD_SEED, DoodadsRuntimeSettings};
pub use spawn::{DoodadRenderIndex, spawn_doodad_render_entity};
pub use sync::DoodadRuntimeSystems;

/// Owns the Doodad Runtime Layer.
pub struct DoodadsRuntimePlugin;

impl Plugin for DoodadsRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DoodadsRuntimeSettings>()
            .register_type::<DoodadRenderEntity>()
            .init_resource::<DoodadsRuntimeSettings>()
            .init_resource::<DoodadRenderIndex>();

        #[cfg(feature = "dev")]
        app.init_resource::<procgen::DevProceduralMaterializationLedger>();

        app.add_systems(Startup, init_doodad_scene_assets)
            .add_systems(
                Update,
                (
                    #[cfg(feature = "dev")]
                    procgen::materialize_dev_procedural_doodads,
                    sync::sync_doodad_render_entities,
                )
                    .chain()
                    .in_set(DoodadRuntimeSystems),
            );
    }
}

fn init_doodad_scene_assets(
    catalog: Res<crate::world::DoodadCatalog>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.insert_resource(preload_doodad_scenes(&catalog, &asset_server));
}
