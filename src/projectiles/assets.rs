use std::collections::HashSet;

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

/// Root folder for projectile glTF assets (ADR-060 C7).
pub const PROJECTILE_ASSET_ROOT: &str = "projectiles";

/// glTF scene index loaded for each projectile key.
pub const DEFAULT_GLTF_SCENE_INDEX: usize = 0;

/// Maps projectile asset keys to preloaded glTF scene handles.
#[derive(Debug, Resource, Default)]
pub struct ProjectileSceneAssets {
    scenes: std::collections::HashMap<String, Handle<Scene>>,
    missing_keys: HashSet<String>,
}

impl ProjectileSceneAssets {
    pub fn ensure_scene(
        &mut self,
        projectile_key: &str,
        asset_server: &AssetServer,
    ) -> Option<Handle<Scene>> {
        if let Some(scene) = self.scenes.get(projectile_key) {
            return Some(scene.clone());
        }
        let path = format!("{PROJECTILE_ASSET_ROOT}/{projectile_key}.glb");
        let scene: Handle<Scene> = asset_server.load(
            GltfAssetLabel::Scene(DEFAULT_GLTF_SCENE_INDEX).from_asset(path),
        );
        self.scenes
            .insert(projectile_key.to_string(), scene.clone());
        Some(scene)
    }

    pub fn log_missing_once(&mut self, key: &str) {
        if self.missing_keys.insert(key.to_owned()) {
            warn!(
                "projectile glTF missing for key `{key}` (expected under {PROJECTILE_ASSET_ROOT}/)"
            );
        }
    }

    #[cfg(test)]
    pub fn from_test_scenes(scenes: std::collections::HashMap<String, Handle<Scene>>) -> Self {
        Self {
            scenes,
            missing_keys: HashSet::new(),
        }
    }
}
