//! Dev scene snapshots — WorldData authoring persistence (ADR-045).

mod actions;
mod load;
mod registry;
mod save;
mod snapshot;

pub use actions::{
    DevSceneRegistry, clear_dev_world, delete_scene, init_dev_scene_registry, load_scene_by_id,
    save_current_world,
};
pub use load::{SceneApplyReport, apply_scene, clear_world_entities};
pub use registry::{SceneCaptureContext, SceneRegistry, SceneRegistryEntry};
pub use save::DEV_SCENES_DIR;
pub use snapshot::{SceneDebugFlagsSnapshot, capture_scene};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId,
        DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitSource, WorldData, WorldPosition, create_doodad, create_unit,
    };
    use bevy::prelude::Vec3;

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

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn scene_capture_produces_deterministic_output() {
        let mut world = flat_world();
        let catalog = UnitCatalog::default();
        create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Dev,
        )
        .unwrap();
        create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("deer"),
            pos(20.0, 20.0),
            UnitSource::Dev,
        )
        .unwrap();
        let ctx = SceneCaptureContext {
            name: "deterministic".into(),
            description: String::new(),
            tags: Vec::new(),
            created_at: 5,
            world_seed: 3,
            camera_state: None,
            debug_flags: None,
        };
        let a = capture_scene(&world, &ctx);
        let b = capture_scene(&world, &ctx);
        assert_eq!(a, b);
        assert_eq!(a.unit_records[0].id, a.unit_records[0].id);
        assert!(a.unit_records[0].id < a.unit_records[1].id);
    }

    #[test]
    fn scene_ordering_is_deterministic() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Dev,
        )
        .unwrap();
        create_doodad(
            &doodad_catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(2.0, 2.0),
            DoodadSource::Dev,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();
        let ctx = SceneCaptureContext {
            name: "order".into(),
            description: String::new(),
            tags: Vec::new(),
            created_at: 1,
            world_seed: 0,
            camera_state: None,
            debug_flags: None,
        };
        let scene = capture_scene(&world, &ctx);
        let unit_ids: Vec<_> = scene.unit_records.iter().map(|u| u.id).collect();
        let doodad_ids: Vec<_> = scene.doodad_records.iter().map(|d| d.id).collect();
        assert_eq!(unit_ids, vec![unit_ids[0]]);
        assert_eq!(doodad_ids, vec![doodad_ids[0]]);
        assert!(
            super::save::scene_to_ron(&scene)
                .unwrap()
                .contains("version")
        );
    }

    #[test]
    fn no_ecs_only_state_is_serialized() {
        let world = flat_world();
        let ctx = SceneCaptureContext {
            name: "pure".into(),
            description: String::new(),
            tags: Vec::new(),
            created_at: 0,
            world_seed: 0,
            camera_state: None,
            debug_flags: None,
        };
        let scene = capture_scene(&world, &ctx);
        let text = super::save::scene_to_ron(&scene)
            .unwrap()
            .to_ascii_lowercase();
        for forbidden in ["entity", "component", "query", "handle", "mesh"] {
            assert!(
                !text.contains(forbidden),
                "scene ron must not contain ecs token `{forbidden}`"
            );
        }
    }
}
