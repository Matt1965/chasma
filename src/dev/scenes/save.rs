//! Persist dev scenes to RON files (ADR-045).

use std::fs;
use std::path::{Path, PathBuf};

use super::snapshot::SceneDefinition;

/// Default on-disk directory for dev scenes (not part of simulation).
pub const DEV_SCENES_DIR: &str = "dev_scenes";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneSaveError {
    Io(String),
    Ron(String),
}

impl std::fmt::Display for SceneSaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::Ron(msg) => write!(f, "ron error: {msg}"),
        }
    }
}

/// Serialize a scene to pretty RON.
pub fn scene_to_ron(scene: &SceneDefinition) -> Result<String, SceneSaveError> {
    ron::ser::to_string_pretty(scene, ron::ser::PrettyConfig::default())
        .map_err(|err| SceneSaveError::Ron(err.to_string()))
}

/// Parse a scene from RON text.
pub fn scene_from_ron(text: &str) -> Result<SceneDefinition, SceneSaveError> {
    ron::from_str(text).map_err(|err| SceneSaveError::Ron(err.to_string()))
}

/// Write a scene file to `dir/{scene_id}.ron`.
pub fn write_scene_file(dir: &Path, scene: &SceneDefinition) -> Result<PathBuf, SceneSaveError> {
    fs::create_dir_all(dir).map_err(|err| SceneSaveError::Io(err.to_string()))?;
    let path = dir.join(format!("{}.ron", scene.scene_id));
    let text = scene_to_ron(scene)?;
    fs::write(&path, text).map_err(|err| SceneSaveError::Io(err.to_string()))?;
    Ok(path)
}

/// Read a scene file from disk.
pub fn read_scene_file(path: &Path) -> Result<SceneDefinition, SceneSaveError> {
    let text = fs::read_to_string(path).map_err(|err| SceneSaveError::Io(err.to_string()))?;
    scene_from_ron(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec3;
    use crate::dev::scenes::{capture_scene, SceneCaptureContext};
    use crate::world::{
        create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitCatalog, UnitDefinitionId, UnitSource, WorldData, WorldPosition,
    };

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
    fn ron_roundtrip_is_stable() {
        let mut world = flat_world();
        let catalog = UnitCatalog::default();
        create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(4.0, 0.0, 4.0)),
            ),
            UnitSource::Dev,
        )
        .unwrap();
        let ctx = SceneCaptureContext {
            name: "roundtrip".into(),
            description: "desc".into(),
            tags: vec!["test".into()],
            created_at: 1,
            world_seed: 99,
            camera_state: None,
            debug_flags: None,
        };
        let scene = capture_scene(&world, &ctx);
        let text = scene_to_ron(&scene).unwrap();
        assert!(!text.contains("Entity"));
        let parsed = scene_from_ron(&text).unwrap();
        assert_eq!(parsed, scene);
    }
}
