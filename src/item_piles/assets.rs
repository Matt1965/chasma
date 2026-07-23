use std::collections::{HashMap, HashSet};

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::world::{ItemCatalog, ItemDefinitionId, ItemRenderKey};

/// Root folder for item world glTF assets (IA0).
///
/// [`ItemRenderKey`] values from import are bare asset stems (`iron_ore`), not
/// `items/iron_ore`. Runtime resolves `assets/items/{key}.glb`.
pub const ITEM_ASSET_ROOT: &str = "items";

/// glTF scene index loaded for each definition (Scene 0 until per-definition override exists).
pub const DEFAULT_GLTF_SCENE_INDEX: usize = 0;

/// Maps catalog definitions to preloaded glTF scene handles.
#[derive(Debug, Resource, Default)]
pub struct ItemSceneAssets {
    scenes: HashMap<ItemDefinitionId, Handle<Scene>>,
    missing_keys: HashSet<String>,
}

impl ItemSceneAssets {
    pub fn scene_for(&self, definition_id: &ItemDefinitionId) -> Option<&Handle<Scene>> {
        self.scenes.get(definition_id)
    }

    /// Return a scene handle for this definition, loading and caching if needed.
    pub fn ensure_scene(
        &mut self,
        definition_id: &ItemDefinitionId,
        render_key: &ItemRenderKey,
        asset_server: &AssetServer,
    ) -> Option<Handle<Scene>> {
        if let Some(scene) = self.scenes.get(definition_id) {
            return Some(scene.clone());
        }
        let Some(path) = gltf_asset_path(render_key) else {
            return None;
        };
        let scene: Handle<Scene> =
            asset_server.load(GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path));
        self.scenes.insert(definition_id.clone(), scene.clone());
        Some(scene)
    }

    pub fn log_missing_once(&mut self, key: &str) {
        if self.missing_keys.insert(key.to_owned()) {
            warn!("item glTF missing for render key `{key}` (expected under {ITEM_ASSET_ROOT}/)");
        }
    }

    /// Build scene assets from preloaded handles (unit tests only).
    #[cfg(test)]
    pub fn from_test_scenes(scenes: HashMap<ItemDefinitionId, Handle<Scene>>) -> Self {
        Self {
            scenes,
            missing_keys: HashSet::new(),
        }
    }
}

/// Resolve a render key to an asset path (without scene label).
pub fn gltf_asset_path(render_key: &ItemRenderKey) -> Option<String> {
    render_key.0.as_ref().map(|key| {
        let stem = key
            .strip_prefix("items/")
            .or_else(|| key.strip_prefix("assets/items/"))
            .unwrap_or(key.as_str());
        format!("{ITEM_ASSET_ROOT}/{stem}.glb")
    })
}

/// Preload scene handles for every catalog definition that has a render key.
pub fn preload_item_scenes(catalog: &ItemCatalog, asset_server: &AssetServer) -> ItemSceneAssets {
    let mut scenes = HashMap::new();
    for definition in catalog.definitions() {
        let Some(path) = gltf_asset_path(&definition.render_key) else {
            continue;
        };
        let scene: Handle<Scene> =
            asset_server.load(GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path));
        scenes.insert(definition.id.clone(), scene);
    }
    ItemSceneAssets {
        scenes,
        missing_keys: HashSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ItemRenderKey;

    #[test]
    fn gltf_path_from_bare_render_key() {
        assert_eq!(
            gltf_asset_path(&ItemRenderKey::reserved("iron_ore")),
            Some("items/iron_ore.glb".to_string())
        );
    }

    #[test]
    fn gltf_path_strips_items_prefix() {
        assert_eq!(
            gltf_asset_path(&ItemRenderKey::reserved("items/iron_ore")),
            Some("items/iron_ore.glb".to_string())
        );
    }

    #[test]
    fn unset_render_key_has_no_path() {
        assert_eq!(gltf_asset_path(&ItemRenderKey::unset()), None);
    }
}
