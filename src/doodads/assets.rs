use std::collections::{HashMap, HashSet};

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::world::{DoodadCatalog, DoodadDefinitionId, DoodadRenderKey};

/// Root folder for doodad glTF assets (ADR-023).
pub const DOODAD_ASSET_ROOT: &str = "doodads";

/// glTF scene index loaded for each definition (Scene 0 until per-definition override exists).
pub const DEFAULT_GLTF_SCENE_INDEX: usize = 0;

/// Maps catalog definitions to preloaded glTF scene handles.
#[derive(Debug, Resource, Default)]
pub struct DoodadSceneAssets {
    scenes: HashMap<DoodadDefinitionId, Handle<Scene>>,
    missing_keys: HashSet<String>,
}

impl DoodadSceneAssets {
    pub fn scene_for(&self, definition_id: &DoodadDefinitionId) -> Option<&Handle<Scene>> {
        self.scenes.get(definition_id)
    }

    /// Return a scene handle for this definition, loading and caching if needed.
    pub fn ensure_scene(
        &mut self,
        definition_id: &DoodadDefinitionId,
        render_key: &DoodadRenderKey,
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
            warn!(
                "doodad glTF missing for render key `{key}` (expected under {DOODAD_ASSET_ROOT}/)"
            );
        }
    }

    /// Build scene assets from preloaded handles (unit tests only).
    #[cfg(test)]
    pub fn from_test_scenes(scenes: HashMap<DoodadDefinitionId, Handle<Scene>>) -> Self {
        Self {
            scenes,
            missing_keys: HashSet::new(),
        }
    }
}

/// Resolve a render key to an asset path (without scene label).
pub fn gltf_asset_path(render_key: &DoodadRenderKey) -> Option<String> {
    render_key
        .0
        .as_ref()
        .map(|key| format!("{DOODAD_ASSET_ROOT}/{key}.glb"))
}

/// Preload scene handles for every catalog definition that has a render key.
pub fn preload_doodad_scenes(
    catalog: &DoodadCatalog,
    asset_server: &AssetServer,
) -> DoodadSceneAssets {
    let mut scenes = HashMap::new();
    for definition in catalog.definitions() {
        let Some(path) = gltf_asset_path(&definition.render_key) else {
            continue;
        };
        let scene: Handle<Scene> =
            asset_server.load(GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path));
        scenes.insert(definition.id.clone(), scene);
    }
    DoodadSceneAssets {
        scenes,
        missing_keys: HashSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::DoodadRenderKey;

    #[test]
    fn gltf_path_from_render_key() {
        assert_eq!(
            gltf_asset_path(&DoodadRenderKey::reserved("tree/oak")),
            Some("doodads/tree/oak.glb".to_string())
        );
        assert_eq!(gltf_asset_path(&DoodadRenderKey::unset()), None);
    }
}
