use std::collections::{HashMap, HashSet};

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::world::{AppearanceProfileCatalog, UnitCatalog, UnitRenderKey};
#[cfg(test)]
use crate::world::UnitDefinitionId;

/// Root folder for unit glTF assets (ADR-028).
///
/// [`UnitRenderKey`] values from Excel import are bare asset stems (`wolf`), not
/// `units/wolf`. Runtime resolves `assets/units/{key}.glb`.
pub const UNIT_ASSET_ROOT: &str = "units";

/// glTF scene index loaded for each definition (Scene 0 until per-definition override exists).
pub const DEFAULT_GLTF_SCENE_INDEX: usize = 0;

/// Maps render keys to preloaded glTF scene handles.
#[derive(Debug, Resource, Default)]
pub struct UnitSceneAssets {
    scenes_by_render_key: HashMap<String, Handle<Scene>>,
    missing_keys: HashSet<String>,
}

impl UnitSceneAssets {
    pub fn scene_for_render_key(&self, render_key: &str) -> Option<&Handle<Scene>> {
        self.scenes_by_render_key.get(render_key)
    }

    /// Test helper — resolves via definition legacy render key.
    #[cfg(test)]
    pub fn scene_for(&self, definition_id: &UnitDefinitionId) -> Option<&Handle<Scene>> {
        let _ = definition_id;
        None
    }

    pub fn log_missing_once(&mut self, key: &str) {
        if self.missing_keys.insert(key.to_owned()) {
            warn!("unit glTF missing for render key `{key}` (expected under {UNIT_ASSET_ROOT}/)");
        }
    }

    /// Build scene assets from preloaded handles (unit tests only).
    #[cfg(test)]
    pub fn from_test_scenes(scenes: HashMap<UnitDefinitionId, Handle<Scene>>) -> Self {
        let scenes_by_render_key = scenes
            .into_iter()
            .map(|(id, handle)| (id.as_str().to_string(), handle))
            .collect();
        Self {
            scenes_by_render_key,
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

/// Preload scene handles for every catalog render key and appearance body variant.
pub fn preload_unit_scenes(
    catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    asset_server: &AssetServer,
) -> UnitSceneAssets {
    let mut keys = HashSet::new();
    for definition in catalog.definitions() {
        if let Some(key) = definition.render_key.0.as_ref() {
            keys.insert(key.clone());
        }
    }
    for profile in appearance_profiles.definitions() {
        for variant in &profile.body_variants {
            if let Some(key) = variant.render_key.0.as_ref() {
                keys.insert(key.clone());
            }
        }
    }

    let mut scenes_by_render_key = HashMap::new();
    for key in keys {
        let path = format!("{UNIT_ASSET_ROOT}/{key}.glb");
        let scene: Handle<Scene> =
            asset_server.load(GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path));
        scenes_by_render_key.insert(key, scene);
    }

    UnitSceneAssets {
        scenes_by_render_key,
        missing_keys: HashSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitRenderKey;

    #[test]
    fn gltf_asset_path_strips_units_prefix() {
        let path = gltf_asset_path(&UnitRenderKey::reserved("units/wolf")).unwrap();
        assert_eq!(path, "units/wolf.glb");
    }

    #[test]
    fn gltf_asset_path_uses_bare_stem() {
        let path = gltf_asset_path(&UnitRenderKey::reserved("wolf")).unwrap();
        assert_eq!(path, "units/wolf.glb");
    }

    #[test]
    fn human_male_and_female_resolve_distinct_gltf_paths() {
        let male = gltf_asset_path(&UnitRenderKey::reserved("human_male")).unwrap();
        let female = gltf_asset_path(&UnitRenderKey::reserved("human_female")).unwrap();
        assert_eq!(male, "units/human_male.glb");
        assert_eq!(female, "units/human_female.glb");
        assert_ne!(male, female);
    }
}
