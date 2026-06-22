//! Authoritative unit terrain grounding (ADR-029 U4).
//!
//! Snaps unit placement Y to resident heightfield samples. Does not use terrain
//! runtime meshes, render exaggeration, or automatic per-frame updates.

use super::id::UnitId;
use super::record::UnitRecord;
use super::UnitInsertError;
use crate::world::ground_world_position;
use crate::world::{WorldData, WorldPosition};

/// Why [`ground_unit_to_terrain`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitGroundingError {
    /// No unit with the given id exists in world data.
    UnitNotFound,
    /// Terrain heightfield for the unit's position is not resident/sampleable.
    TerrainUnavailable,
}

/// Return a copy of `position` with authoritative Y from the resident heightfield.
///
/// X/Z and chunk are unchanged. Returns `None` when terrain is unavailable.
pub fn ground_unit_position(world: &WorldData, position: WorldPosition) -> Option<WorldPosition> {
    ground_world_position(world, position)
}

/// Snap an existing unit's placement Y to resident terrain height.
///
/// On [`UnitGroundingError::TerrainUnavailable`], the unit record is not modified.
pub fn ground_unit_to_terrain(
    world: &mut WorldData,
    unit_id: UnitId,
) -> Result<UnitRecord, UnitGroundingError> {
    let position = world
        .get_unit(unit_id)
        .ok_or(UnitGroundingError::UnitNotFound)?
        .placement
        .position;
    let grounded =
        ground_unit_position(world, position).ok_or(UnitGroundingError::TerrainUnavailable)?;
    world.relocate_unit(unit_id, grounded).map_err(|error| match error {
        UnitInsertError::UnitNotFound => UnitGroundingError::UnitNotFound,
        UnitInsertError::ChunkPlacementMismatch => UnitGroundingError::TerrainUnavailable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
    use crate::units::{
        sync_unit_render_entities, UnitRenderIndex, UnitSceneAssets, UnitSyncOverrides,
    };
    use crate::world::{
        create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitCatalog, UnitDefinitionId, UnitMetadata, UnitPlacement, UnitRecord, UnitSource,
        UnitState, WorldConfig, WorldData,
    };
    use bevy::asset::AssetPlugin;
    use bevy::prelude::{
        App, Assets, MinimalPlugins, Quat, Scene, StandardMaterial, Transform, Update, Vec3, World,
    };
    use std::collections::HashMap;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn insert_terrain(world: &mut WorldData, x: i32, z: i32, height: f32) -> ChunkId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        let samples = vec![height; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(chunk, ChunkData::new(heightfield, Vec::new()));
        chunk
    }

    fn authored_unit_at(world: &mut WorldData, x: i32, z: i32, y: f32) -> UnitId {
        let catalog = UnitCatalog::default();
        create_unit(
            &catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            WorldPosition::new(
                ChunkCoord::new(x, z),
                LocalPosition::new(Vec3::new(64.0, y, 128.0)),
            ),
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    #[test]
    fn ground_unit_position_samples_resident_terrain() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 0, 0, 14.0);
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(64.0, 99.0, 128.0)),
        );
        let grounded = ground_unit_position(&world, position).unwrap();
        assert_eq!(grounded.local.0.x, 64.0);
        assert_eq!(grounded.local.0.z, 128.0);
        assert_eq!(grounded.local.0.y, 14.0);
    }

    #[test]
    fn grounding_updates_unit_y() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 1, 2, 20.0);
        let unit_id = authored_unit_at(&mut world, 1, 2, 0.0);

        let updated = ground_unit_to_terrain(&mut world, unit_id).unwrap();
        assert_eq!(updated.placement.position.local.0.y, 20.0);
    }

    #[test]
    fn xz_unchanged_after_grounding() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 0, 0, 5.0);
        let unit_id = authored_unit_at(&mut world, 0, 0, 100.0);

        ground_unit_to_terrain(&mut world, unit_id).unwrap();
        let local = world.get_unit(unit_id).unwrap().placement.position.local.0;
        assert_eq!(local.x, 64.0);
        assert_eq!(local.z, 128.0);
    }

    #[test]
    fn rotation_preserved_after_grounding() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 0, 0, 5.0);
        let unit_id = world.allocate_unit_id();
        let rotation = Quat::from_rotation_y(0.75);
        let record = UnitRecord::new(
            unit_id,
            UnitDefinitionId::new("wolf"),
            UnitPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(64.0, 50.0, 128.0)),
                ),
                rotation,
            ),
            UnitSource::Authored,
        );
        world
            .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();

        ground_unit_to_terrain(&mut world, unit_id).unwrap();
        assert_eq!(
            world.get_unit(unit_id).unwrap().placement.rotation,
            rotation
        );
    }

    #[test]
    fn state_source_metadata_preserved_after_grounding() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 0, 0, 5.0);
        let unit_id = world.allocate_unit_id();
        let source = UnitSource::Procedural { seed: 99 };
        let mut record = UnitRecord::new(
            unit_id,
            UnitDefinitionId::new("wolf"),
            UnitPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(64.0, 50.0, 128.0)),
                ),
                Quat::IDENTITY,
            ),
            source,
        );
        record.state = UnitState::Idle;
        record.metadata = UnitMetadata;
        world
            .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();

        ground_unit_to_terrain(&mut world, unit_id).unwrap();
        let updated = world.get_unit(unit_id).unwrap();
        assert_eq!(updated.state, UnitState::Idle);
        assert_eq!(updated.source, source);
        assert_eq!(updated.metadata, UnitMetadata);
    }

    #[test]
    fn missing_terrain_returns_error_without_mutation() {
        let mut world = WorldData::new(layout());
        let unit_id = authored_unit_at(&mut world, 0, 0, 42.0);

        let err = ground_unit_to_terrain(&mut world, unit_id).unwrap_err();
        assert_eq!(err, UnitGroundingError::TerrainUnavailable);
        assert_eq!(
            world.get_unit(unit_id).unwrap().placement.position.local.0.y,
            42.0
        );
    }

    #[test]
    fn missing_unit_returns_not_found() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 0, 0, 5.0);
        let err = ground_unit_to_terrain(&mut world, UnitId::new(999)).unwrap_err();
        assert_eq!(err, UnitGroundingError::UnitNotFound);
    }

    #[test]
    fn grounding_works_after_arbitrary_authoring_y() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 3, 4, 18.5);
        let unit_id = authored_unit_at(&mut world, 3, 4, -500.0);

        ground_unit_to_terrain(&mut world, unit_id).unwrap();
        assert_eq!(
            world.get_unit(unit_id).unwrap().placement.position.local.0.y,
            18.5
        );
    }

    #[test]
    fn render_sync_observes_grounded_y_with_vertical_scale() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<UnitCatalog>();
        app.init_resource::<crate::terrain::ChunkResidencyTracker>();
        app.init_resource::<UnitRenderIndex>();
        app.init_resource::<Assets<Scene>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.insert_resource(UnitSyncOverrides {
            treat_scenes_loaded: true,
        });
        app.add_systems(Update, sync_unit_render_entities);

        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(World::new()))
        };
        app.insert_resource(UnitSceneAssets::from_test_scenes(HashMap::from([(
            UnitDefinitionId::new("wolf"),
            scene,
        )])));

        let vertical_scale = 3.0;
        let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial::default());
        app.world_mut().insert_resource(TerrainRenderAssets {
            material,
            vertical_scale,
        });

        let unit_id = {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 0, 0, 10.0);
            authored_unit_at(&mut world, 0, 0, 0.0)
        };
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            ground_unit_to_terrain(&mut world, unit_id).unwrap();
        }

        app.world_mut()
            .resource_mut::<crate::terrain::ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.update();

        let record = app.world().resource::<WorldData>().get_unit(unit_id).unwrap();
        assert_eq!(record.placement.position.local.0.y, 10.0);

        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        let transform = app.world().entity(entity).get::<Transform>().unwrap();
        let config = app.world().resource::<WorldConfig>();
        let expected = world_position_to_render_global(
            record.placement.position,
            config.chunk_layout(),
            vertical_scale,
        );
        assert_eq!(transform.translation, expected);
        assert_eq!(transform.translation.y, 10.0 * vertical_scale);
    }
}
