//! Dev scene registry — local file index (ADR-045).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::save::{DEV_SCENES_DIR, SceneSaveError, read_scene_file, write_scene_file};
use super::snapshot::SceneDefinition;

const REGISTRY_VERSION: u32 = 1;
const INDEX_FILE: &str = "index.ron";

/// Metadata for one registered scene (index entry only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneRegistryEntry {
    pub scene_id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub file_name: String,
}

/// On-disk registry index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneRegistryIndex {
    pub version: u32,
    pub entries: Vec<SceneRegistryEntry>,
}

impl Default for SceneRegistryIndex {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            entries: Vec::new(),
        }
    }
}

/// Dev scene catalog stored outside simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneRegistry {
    dir: PathBuf,
    index: SceneRegistryIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneRegistryError {
    Save(SceneSaveError),
    DuplicateId(String),
    NotFound(String),
}

impl std::fmt::Display for SceneRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Save(err) => write!(f, "{err}"),
            Self::DuplicateId(id) => write!(f, "scene id already registered: {id}"),
            Self::NotFound(id) => write!(f, "scene not found: {id}"),
        }
    }
}

impl SceneRegistry {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            index: SceneRegistryIndex::default(),
        }
    }

    pub fn with_default_dir() -> Self {
        Self::new(DEV_SCENES_DIR)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn entries(&self) -> &[SceneRegistryEntry] {
        &self.index.entries
    }

    pub fn list(&self) -> &[SceneRegistryEntry] {
        self.entries()
    }

    pub fn search(&self, query: &str) -> Vec<&SceneRegistryEntry> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return self.index.entries.iter().collect();
        }
        self.index
            .entries
            .iter()
            .filter(|entry| {
                entry.name.to_ascii_lowercase().contains(&q)
                    || entry.description.to_ascii_lowercase().contains(&q)
                    || entry
                        .tags
                        .iter()
                        .any(|tag| tag.to_ascii_lowercase().contains(&q))
            })
            .collect()
    }

    pub fn get(&self, scene_id: &str) -> Option<&SceneRegistryEntry> {
        self.index
            .entries
            .iter()
            .find(|entry| entry.scene_id == scene_id)
    }

    pub fn load_from_disk(&mut self) -> Result<(), SceneSaveError> {
        fs::create_dir_all(&self.dir).map_err(|err| SceneSaveError::Io(err.to_string()))?;
        let index_path = self.dir.join(INDEX_FILE);
        if !index_path.exists() {
            self.index = SceneRegistryIndex::default();
            return Ok(());
        }
        let text =
            fs::read_to_string(&index_path).map_err(|err| SceneSaveError::Io(err.to_string()))?;
        self.index = ron::from_str(&text).map_err(|err| SceneSaveError::Ron(err.to_string()))?;
        self.index
            .entries
            .sort_by(|a, b| a.scene_id.cmp(&b.scene_id));
        Ok(())
    }

    pub fn save_index(&self) -> Result<(), SceneSaveError> {
        fs::create_dir_all(&self.dir).map_err(|err| SceneSaveError::Io(err.to_string()))?;
        let mut index = self.index.clone();
        index.entries.sort_by(|a, b| a.scene_id.cmp(&b.scene_id));
        let text = ron::ser::to_string_pretty(&index, ron::ser::PrettyConfig::default())
            .map_err(|err| SceneSaveError::Ron(err.to_string()))?;
        fs::write(self.dir.join(INDEX_FILE), text)
            .map_err(|err| SceneSaveError::Io(err.to_string()))
    }

    pub fn register(
        &mut self,
        scene: SceneDefinition,
    ) -> Result<SceneRegistryEntry, SceneRegistryError> {
        if self.get(&scene.scene_id).is_some() {
            return Err(SceneRegistryError::DuplicateId(scene.scene_id.clone()));
        }
        write_scene_file(&self.dir, &scene).map_err(SceneRegistryError::Save)?;
        let entry = SceneRegistryEntry {
            scene_id: scene.scene_id.clone(),
            name: scene.name.clone(),
            description: scene.description.clone(),
            tags: scene.tags.clone(),
            created_at: scene.created_at,
            file_name: format!("{}.ron", scene.scene_id),
        };
        self.index.entries.push(entry.clone());
        self.index
            .entries
            .sort_by(|a, b| a.scene_id.cmp(&b.scene_id));
        self.save_index().map_err(SceneRegistryError::Save)?;
        Ok(entry)
    }

    pub fn upsert(
        &mut self,
        scene: SceneDefinition,
    ) -> Result<SceneRegistryEntry, SceneRegistryError> {
        if let Some(index) = self
            .index
            .entries
            .iter()
            .position(|e| e.scene_id == scene.scene_id)
        {
            write_scene_file(&self.dir, &scene).map_err(SceneRegistryError::Save)?;
            let entry = &mut self.index.entries[index];
            entry.name = scene.name.clone();
            entry.description = scene.description.clone();
            entry.tags = scene.tags.clone();
            entry.created_at = scene.created_at;
            self.save_index().map_err(SceneRegistryError::Save)?;
            return Ok(self.index.entries[index].clone());
        }
        self.register(scene)
    }

    pub fn delete(&mut self, scene_id: &str) -> Result<(), SceneRegistryError> {
        let entry = self
            .get(scene_id)
            .cloned()
            .ok_or_else(|| SceneRegistryError::NotFound(scene_id.to_string()))?;
        let path = self.dir.join(&entry.file_name);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|err| SceneRegistryError::Save(SceneSaveError::Io(err.to_string())))?;
        }
        self.index.entries.retain(|e| e.scene_id != scene_id);
        self.save_index().map_err(SceneRegistryError::Save)?;
        Ok(())
    }

    pub fn load_scene(&self, scene_id: &str) -> Result<SceneDefinition, SceneRegistryError> {
        let entry = self
            .get(scene_id)
            .ok_or_else(|| SceneRegistryError::NotFound(scene_id.to_string()))?;
        read_scene_file(&self.dir.join(&entry.file_name)).map_err(SceneRegistryError::Save)
    }
}

/// Context passed into scene capture (dev-only metadata).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneCaptureContext {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub world_seed: u64,
    pub camera_state: Option<super::snapshot::SceneCameraState>,
    pub debug_flags: Option<super::snapshot::SceneDebugFlagsSnapshot>,
}

impl SceneCaptureContext {
    pub fn from_dev_state(
        name: impl Into<String>,
        description: impl Into<String>,
        world_seed: u64,
        debug_flags: Option<super::snapshot::SceneDebugFlagsSnapshot>,
        camera_state: Option<super::snapshot::SceneCameraState>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            tags: Vec::new(),
            created_at: unix_timestamp_secs(),
            world_seed,
            camera_state,
            debug_flags,
        }
    }
}

pub fn make_scene_id(name: &str, created_at: u64) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_');
    let slug = if slug.is_empty() { "scene" } else { slug };
    format!("{slug}_{created_at}")
}

pub fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::scenes::capture_scene;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitSource, WorldData, WorldPosition, create_unit,
    };

    fn temp_registry() -> (SceneRegistry, PathBuf) {
        let dir = std::env::temp_dir().join(format!("chasma_scenes_{}", uuid_simple()));
        let registry = SceneRegistry::new(&dir);
        (registry, dir)
    }

    fn uuid_simple() -> u64 {
        unix_timestamp_secs()
    }

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    #[test]
    fn registry_register_list_delete() {
        let (mut registry, dir) = temp_registry();
        let world = flat_world();
        let ctx = SceneCaptureContext {
            name: "Arena".into(),
            description: "test arena".into(),
            tags: vec!["pvp".into()],
            created_at: 100,
            world_seed: 1,
            camera_state: None,
            debug_flags: None,
        };
        let scene = capture_scene(&world, &ctx);
        registry.register(scene).unwrap();
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.search("arena").len(), 1);
        registry.delete(&make_scene_id("Arena", 100)).unwrap();
        assert!(registry.list().is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
