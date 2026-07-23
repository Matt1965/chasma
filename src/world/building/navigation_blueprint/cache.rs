//! Cache manifest for navigation blueprint generation (NV1.2).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::id::BuildingNavigationBlueprintId;

/// Bump when generation algorithm or output semantics change.
pub const NAVIGATION_BLUEPRINT_GENERATOR_VERSION: u32 = 1;

pub const NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH: &str =
    "assets/buildings/navigation_blueprints/cache_manifest.ron";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NavigationBlueprintCacheManifest {
    pub generator_version: u32,
    #[serde(default)]
    pub entries: Vec<NavigationBlueprintCacheEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavigationBlueprintCacheEntry {
    pub blueprint_id: String,
    pub building_definition_id: String,
    pub collision_render_key: String,
    pub collision_source_hash: String,
    #[serde(default)]
    pub render_source_hash: Option<String>,
    #[serde(default)]
    pub baseline_scale_milli: Option<i32>,
}

impl NavigationBlueprintCacheManifest {
    pub fn load_from_path(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| ron::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|err| err.to_string())?;
        std::fs::write(path, text).map_err(|err| err.to_string())
    }

    pub fn entry_map(&self) -> BTreeMap<String, NavigationBlueprintCacheEntry> {
        self.entries
            .iter()
            .map(|entry| (entry.blueprint_id.clone(), entry.clone()))
            .collect()
    }

    pub fn upsert(&mut self, entry: NavigationBlueprintCacheEntry) {
        self.generator_version = NAVIGATION_BLUEPRINT_GENERATOR_VERSION;
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.blueprint_id == entry.blueprint_id)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    pub fn is_fresh(
        &self,
        blueprint_id: &BuildingNavigationBlueprintId,
        collision_hash: &str,
        render_hash: Option<&str>,
        baseline_scale_milli: Option<i32>,
    ) -> bool {
        if self.generator_version != NAVIGATION_BLUEPRINT_GENERATOR_VERSION {
            return false;
        }
        let Some(entry) = self.entries.iter().find(|e| e.blueprint_id == blueprint_id.as_str())
        else {
            return false;
        };
        entry.collision_source_hash == collision_hash
            && entry.render_source_hash.as_deref() == render_hash
            && entry.baseline_scale_milli == baseline_scale_milli
    }
}
