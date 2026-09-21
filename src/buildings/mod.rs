//! Building Runtime Layer (ADR-079 B2, ADR-095 BA1).
//!
//! Owns derived, disposable runtime concerns for buildings: glTF scene entities,
//! diagnostic fallbacks, and sync with terrain residency. Authoritative instance
//! data remains in [`crate::world::WorldData`].

use bevy::prelude::*;

pub mod assets;
pub mod components;
pub mod foundation;
pub mod foundation_assets;
pub mod foundation_diagnostic;
pub mod fallback;
pub mod picking;
pub mod placeholder;
pub mod presentation;
pub mod scene_materials;
pub mod spawn;
pub mod sync;

pub use assets::{
    BUILDING_ASSET_ROOT, BuildingSceneAssets, DEFAULT_GLTF_SCENE_INDEX, ghost_render_key,
    gltf_asset_path, lifecycle_render_key, preload_building_scenes,
};
pub use components::{
    BuildingDiagnosticFallback, BuildingFoundationSkirt, BuildingRenderEntity, BuildingSceneRoot,
    BuildingSceneTags, OriginalBuildingMaterial,
};
pub use fallback::{BuildingFallbackAssets, BuildingFallbackReason};
pub use picking::pick_building_along_ray;
pub use spawn::{
    BuildingRenderIndex, building_render_translation, despawn_building_render_entities,
    spawn_building_scene_entity,
};
pub use sync::BuildingRuntimeSystems;

/// Owns the Building Runtime Layer.
pub struct BuildingsRuntimePlugin;

impl Plugin for BuildingsRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<BuildingRenderEntity>()
            .register_type::<BuildingSceneRoot>()
            .register_type::<BuildingDiagnosticFallback>()
            .register_type::<BuildingSceneTags>()
            .register_type::<OriginalBuildingMaterial>()
            .register_type::<BuildingFallbackReason>()
            .init_resource::<BuildingRenderIndex>()
            .init_resource::<BuildingFallbackAssets>()
            .add_systems(
                Startup,
                (
                    init_building_scene_assets,
                    foundation_assets::init_foundation_presentation_assets,
                ),
            )
            .add_systems(
                Update,
                (
                    sync::sync_building_render_entities,
                    presentation::discover_building_scene_tags,
                    presentation::apply_building_lifecycle_tints,
                    presentation::sync_building_fallback_materials,
                    sync::sync_building_foundation_skirts,
                    foundation_assets::sync_foundation_texture_binding,
                    foundation_diagnostic::log_foundation_runtime_once,
                )
                    .chain()
                    .in_set(BuildingRuntimeSystems),
            );
    }
}

fn init_building_scene_assets(
    catalog: Res<crate::world::BuildingCatalog>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.insert_resource(preload_building_scenes(&catalog, &asset_server));
}
