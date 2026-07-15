//! Shared glTF scene cache for building presentation (ADR-095 BA1).

use std::collections::{HashMap, HashSet};

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, BuildingDefinition, BuildingLifecycleState, BuildingRenderKey,
};

/// Root folder for building glTF assets (ADR-078, ADR-095).
pub const BUILDING_ASSET_ROOT: &str = "buildings";

/// glTF scene index until per-definition scene overrides exist.
pub const DEFAULT_GLTF_SCENE_INDEX: usize = 0;

/// Maps render keys to preloaded glTF scene handles (shared across instances).
#[derive(Debug, Resource, Default)]
pub struct BuildingSceneAssets {
    scenes: HashMap<String, Handle<Scene>>,
    missing_keys: HashSet<String>,
    failed_keys: HashSet<String>,
}

impl BuildingSceneAssets {
    pub fn scene_for_key(&self, render_key: &str) -> Option<&Handle<Scene>> {
        self.scenes.get(render_key)
    }

    /// Return a cached scene handle for `render_key`, loading once if needed.
    pub fn ensure_scene(
        &mut self,
        render_key: &str,
        asset_server: &AssetServer,
    ) -> Option<Handle<Scene>> {
        if let Some(scene) = self.scenes.get(render_key) {
            return Some(scene.clone());
        }
        let path = format!("{BUILDING_ASSET_ROOT}/{render_key}.glb");
        let scene: Handle<Scene> =
            asset_server.load(GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path));
        self.scenes.insert(render_key.to_owned(), scene.clone());
        Some(scene)
    }

    pub fn log_missing_once(&mut self, key: &str) {
        if self.missing_keys.insert(key.to_owned()) {
            warn!(
                "building glTF missing for render key `{key}` (expected `assets/{BUILDING_ASSET_ROOT}/{key}.glb`)"
            );
        }
    }

    pub fn log_failed_once(&mut self, key: &str) {
        if self.failed_keys.insert(key.to_owned()) {
            warn!("building glTF failed to load for render key `{key}`");
        }
    }

    /// Build scene assets from preloaded handles (unit tests only).
    #[cfg(test)]
    pub fn from_test_scenes(scenes: HashMap<String, Handle<Scene>>) -> Self {
        Self {
            scenes,
            missing_keys: HashSet::new(),
            failed_keys: HashSet::new(),
        }
    }
}

/// Resolve a render key to an asset path (without scene label).
pub fn gltf_asset_path(render_key: &BuildingRenderKey) -> Option<String> {
    render_key.0.as_ref().map(|key| {
        let stem = key
            .strip_prefix("buildings/")
            .or_else(|| key.strip_prefix("assets/buildings/"))
            .unwrap_or(key.as_str());
        format!("{BUILDING_ASSET_ROOT}/{stem}.glb")
    })
}

/// Render key string used for the active lifecycle visual.
///
/// Per-lifecycle GLB overrides are not on [`BuildingDefinition`] yet; all states
/// share `render_key` and use lifecycle tint until optional stage assets exist.
pub fn lifecycle_render_key(
    definition: &BuildingDefinition,
    _lifecycle: BuildingLifecycleState,
) -> Option<String> {
    definition.render_key.0.clone()
}

/// Ghost/preview render key: `preview_render_key` when set, else complete `render_key`.
pub fn ghost_render_key(definition: &BuildingDefinition) -> Option<String> {
    definition
        .preview_render_key
        .as_ref()
        .and_then(|key| key.0.clone())
        .or_else(|| definition.render_key.0.clone())
}

/// Collect unique render keys to preload from the building catalog.
pub fn collect_preload_render_keys(catalog: &BuildingCatalog) -> HashSet<String> {
    let mut keys = HashSet::new();
    for definition in catalog.definitions() {
        if let Some(key) = definition.render_key.0.as_deref() {
            keys.insert(key.to_owned());
        }
        if let Some(key) = definition
            .preview_render_key
            .as_ref()
            .and_then(|value| value.0.as_deref())
        {
            keys.insert(key.to_owned());
        }
    }
    keys
}

/// Preload scene handles for every unique catalog render key.
pub fn preload_building_scenes(
    catalog: &BuildingCatalog,
    asset_server: &AssetServer,
) -> BuildingSceneAssets {
    let mut scenes = HashMap::new();
    for key in collect_preload_render_keys(catalog) {
        let path = format!("{BUILDING_ASSET_ROOT}/{key}.glb");
        let scene: Handle<Scene> =
            asset_server.load(GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path));
        scenes.insert(key, scene);
    }
    BuildingSceneAssets {
        scenes,
        missing_keys: HashSet::new(),
        failed_keys: HashSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCategoryId, BuildingDefinition, BuildingDefinitionId, FootprintSpec,
    };

    fn hut_definition() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            250,
            45.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )
    }

    #[test]
    fn gltf_path_from_bare_render_key() {
        assert_eq!(
            gltf_asset_path(&BuildingRenderKey::reserved("hut")),
            Some("buildings/hut.glb".to_string())
        );
    }

    #[test]
    fn gltf_path_strips_buildings_prefix() {
        assert_eq!(
            gltf_asset_path(&BuildingRenderKey::reserved("buildings/barn")),
            Some("buildings/barn.glb".to_string())
        );
    }

    #[test]
    fn lifecycle_render_key_uses_complete_asset() {
        let definition = hut_definition();
        assert_eq!(
            lifecycle_render_key(&definition, BuildingLifecycleState::Planned),
            Some("hut".to_string())
        );
        assert_eq!(
            lifecycle_render_key(&definition, BuildingLifecycleState::Complete),
            Some("hut".to_string())
        );
    }

    #[test]
    fn ghost_render_key_prefers_preview() {
        let definition =
            hut_definition().with_preview_render_key(BuildingRenderKey::reserved("hut_preview"));
        assert_eq!(
            ghost_render_key(&definition),
            Some("hut_preview".to_string())
        );
    }

    #[test]
    fn ghost_render_key_falls_back_to_complete() {
        assert_eq!(ghost_render_key(&hut_definition()), Some("hut".to_string()));
    }
}
