use std::collections::{HashMap, HashSet};

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::world::{UnitCatalog, UnitDefinitionId, UnitRenderKey};

/// Root folder for unit glTF assets (ADR-028).
///
/// [`UnitRenderKey`] values from Excel import are bare asset stems (`wolf`), not
/// `units/wolf`. Runtime resolves `assets/units/{key}.glb`.
pub const UNIT_ASSET_ROOT: &str = "units";

/// glTF scene index loaded for each definition (Scene 0 until per-definition override exists).
pub const DEFAULT_GLTF_SCENE_INDEX: usize = 0;

/// Maps catalog definitions to preloaded glTF scene handles.
#[derive(Debug, Resource, Default)]
pub struct UnitSceneAssets {
    scenes: HashMap<UnitDefinitionId, Handle<Scene>>,
    missing_keys: HashSet<String>,
}

impl UnitSceneAssets {
    pub fn scene_for(&self, definition_id: &UnitDefinitionId) -> Option<&Handle<Scene>> {
        self.scenes.get(definition_id)
    }

    pub fn log_missing_once(&mut self, key: &str) {
        if self.missing_keys.insert(key.to_owned()) {
            warn!("unit glTF missing for render key `{key}` (expected under {UNIT_ASSET_ROOT}/)");
        }
    }

    /// Build scene assets from preloaded handles (unit tests only).
    #[cfg(test)]
    pub fn from_test_scenes(scenes: HashMap<UnitDefinitionId, Handle<Scene>>) -> Self {
        Self {
            scenes,
            missing_keys: HashSet::new(),
        }
    }
}

/// Resolve a render key to an asset path (without scene label).
///
/// Import normalization (`normalize_file_path_to_render_key`) stores bare stems
/// such as `wolf`. Legacy keys that still include a `units/` prefix are stripped.
pub fn gltf_asset_path(render_key: &UnitRenderKey) -> Option<String> {
    render_key.0.as_ref().map(|key| {
        let stem = key
            .strip_prefix("units/")
            .or_else(|| key.strip_prefix("assets/units/"))
            .unwrap_or(key.as_str());
        format!("{UNIT_ASSET_ROOT}/{stem}.glb")
    })
}

/// Preload scene handles for every catalog definition that has a render key.
pub fn preload_unit_scenes(catalog: &UnitCatalog, asset_server: &AssetServer) -> UnitSceneAssets {
    let mut scenes = HashMap::new();
    for definition in catalog.definitions() {
        let Some(path) = gltf_asset_path(&definition.render_key) else {
            continue;
        };
        let scene: Handle<Scene> = asset_server.load(
            GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path),
        );
        scenes.insert(definition.id.clone(), scene);
    }
    UnitSceneAssets {
        scenes,
        missing_keys: HashSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitRenderKey;

    #[test]
    fn gltf_path_from_bare_render_key() {
        assert_eq!(
            gltf_asset_path(&UnitRenderKey::reserved("wolf")),
            Some("units/wolf.glb".to_string())
        );
    }

    #[test]
    fn gltf_path_strips_units_prefix() {
        assert_eq!(
            gltf_asset_path(&UnitRenderKey::reserved("units/wolf")),
            Some("units/wolf.glb".to_string())
        );
    }

    #[test]
    fn unset_render_key_has_no_path() {
        assert_eq!(gltf_asset_path(&UnitRenderKey::unset()), None);
    }
}
