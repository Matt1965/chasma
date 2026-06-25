//! Sync unit render entities with authoritative world data and terrain residency (ADR-028).
//!
//! [`WorldData`] stores authoritative placement Y in world units. Render transforms
//! multiply Y by [`TerrainRenderAssets::vertical_scale`] so units align with the
//! visible terrain mesh (ADR-010). World records are never modified.

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{world_position_to_render_global, TerrainRenderAssets};
use crate::world::{UnitCatalog, UnitId, WorldConfig, WorldData};

use super::assets::UnitSceneAssets;
use super::components::UnitRenderEntity;
use super::spawn::{despawn_unit_render_entities, spawn_unit_render_entity, UnitRenderIndex};

/// Systems that sync unit render entities with world data.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct UnitRuntimeSystems;

/// Test-only override for sync integration tests (not inserted in production).
#[derive(Resource, Default, Debug)]
pub struct UnitSyncOverrides {
    pub treat_scenes_loaded: bool,
}

/// Collect unit ids that should have render entities this frame.
pub(crate) fn visible_unit_ids(
    world: &WorldData,
    residency: &ChunkResidencyTracker,
) -> HashSet<UnitId> {
    let mut visible = HashSet::new();
    for (chunk_id, _) in world.iter() {
        if !residency.is_resident(chunk_id) {
            continue;
        }
        let Some(store) = world.units_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            visible.insert(record.id);
        }
    }
    visible
}

/// Keep derived unit entities aligned with [`WorldData`] and terrain residency.
pub fn sync_unit_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    config: Res<WorldConfig>,
    residency: Res<ChunkResidencyTracker>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<UnitSceneAssets>,
    mut index: ResMut<UnitRenderIndex>,
    existing: Query<(Entity, &UnitRenderEntity)>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    overrides: Option<Res<UnitSyncOverrides>>,
) {
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let force_scenes_loaded = overrides
        .as_ref()
        .is_some_and(|value| value.treat_scenes_loaded);
    let should_render = visible_unit_ids(&world, &residency);

    let stale: Vec<UnitId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    despawn_unit_render_entities(&mut commands, &mut index, stale);

    for (entity, marker) in &existing {
        if !should_render.contains(&marker.unit_id) {
            continue;
        }
        let Some(record) = world.get_unit(marker.unit_id) else {
            commands.entity(entity).despawn();
            index.0.remove(&marker.unit_id);
            continue;
        };
        let layout = config.chunk_layout();
        let translation =
            world_position_to_render_global(record.placement.position, layout, vertical_scale);
        commands.entity(entity).insert(Transform {
            translation,
            rotation: record.placement.rotation,
            scale: Vec3::ONE,
        });
    }

    for id in should_render {
        if index.0.contains_key(&id) {
            continue;
        }

        let Some(record) = world.get_unit(id) else {
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            warn!(
                "unit {} references missing definition `{}`",
                record.id.raw(),
                record.definition_id.as_str()
            );
            continue;
        };
        let Some(scene) = scene_assets.scene_for(&definition.id).cloned() else {
            if let Some(key) = definition.render_key.0.as_deref() {
                scene_assets.log_missing_once(key);
            }
            continue;
        };
        if !force_scenes_loaded && !scene_is_loaded(&asset_server, &scene) {
            continue;
        }

        let entity = spawn_unit_render_entity(
            &mut commands,
            record,
            scene,
            &config,
            vertical_scale,
        );
        index.0.insert(id, entity);
    }
}

fn scene_is_loaded(asset_server: &AssetServer, scene: &Handle<Scene>) -> bool {
    matches!(
        asset_server.get_load_state(scene),
        Some(LoadState::Loaded)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
    use crate::units::{UnitSceneAssets, UnitSyncOverrides};
    use crate::world::{
        create_unit, Heightfield, ChunkCoord, ChunkData, ChunkId, ChunkLayout, LocalPosition,
        UnitCatalog, UnitDefinition, UnitDefinitionId, UnitId, UnitRenderKey, UnitSource,
        WorldConfig, WorldData, WorldPosition,
    };
    use bevy::asset::AssetPlugin;
    use bevy::prelude::{App, MinimalPlugins, Quat, StandardMaterial, Update, Vec3};
    use std::collections::HashMap;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn insert_terrain(world: &mut WorldData, x: i32, z: i32) {
        let samples = vec![8.0; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn insert_authored_unit(world: &mut WorldData, catalog: &UnitCatalog, x: i32, z: i32) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            WorldPosition::new(
                ChunkCoord::new(x, z),
                LocalPosition::new(Vec3::new(20.0, 8.0, 30.0)),
            ),
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    fn setup_sync_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<UnitCatalog>();
        app.init_resource::<ChunkResidencyTracker>();
        app.init_resource::<UnitRenderIndex>();
        app.init_resource::<Assets<Scene>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.insert_resource(UnitSyncOverrides {
            treat_scenes_loaded: true,
        });
        app.add_systems(Update, sync_unit_render_entities);
        app
    }

    fn prepare_resident_unit(app: &mut App, x: i32, z: i32) -> UnitId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        let catalog = app.world().resource::<UnitCatalog>().clone();
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(World::new()))
        };
        app.insert_resource(UnitSceneAssets::from_test_scenes(HashMap::from([(
            UnitDefinitionId::new("wolf"),
            scene,
        )])));
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, x, z);
            insert_authored_unit(&mut world, &catalog, x, z)
        };
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.world()
            .resource::<WorldData>()
            .units_in_chunk(chunk)
            .unwrap()
            .records()[0]
            .id
    }

    #[test]
    fn visible_ids_require_resident_terrain() {
        let mut world = WorldData::new(layout());
        let catalog = UnitCatalog::default();
        insert_terrain(&mut world, 0, 0);
        let id = insert_authored_unit(&mut world, &catalog, 0, 0);

        let mut residency = ChunkResidencyTracker::default();
        assert!(visible_unit_ids(&world, &residency).is_empty());

        residency.mark_resident(ChunkId::new(ChunkCoord::new(0, 0)));
        assert_eq!(visible_unit_ids(&world, &residency), HashSet::from([id]));
    }

    #[test]
    fn sync_spawns_render_entity_for_resident_record() {
        let mut app = setup_sync_app();
        let unit_id = prepare_resident_unit(&mut app, 1, 2);
        app.update();

        let index = app.world().resource::<UnitRenderIndex>();
        assert_eq!(index.0.len(), 1);
        assert!(index.0.contains_key(&unit_id));
    }

    #[test]
    fn sync_does_not_spawn_when_chunk_not_resident() {
        let mut app = setup_sync_app();
        let catalog = app.world().resource::<UnitCatalog>().clone();
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(World::new()))
        };
        app.insert_resource(UnitSceneAssets::from_test_scenes(HashMap::from([(
            UnitDefinitionId::new("wolf"),
            scene,
        )])));
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 0, 0);
            insert_authored_unit(&mut world, &catalog, 0, 0);
        }
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
    }

    #[test]
    fn sync_does_not_duplicate_across_ticks() {
        let mut app = setup_sync_app();
        let unit_id = prepare_resident_unit(&mut app, 3, 4);
        app.update();
        app.update();

        let index = app.world().resource::<UnitRenderIndex>();
        assert_eq!(index.0.len(), 1);
        assert!(index.0.contains_key(&unit_id));
    }

    #[test]
    fn sync_despawns_when_chunk_not_resident() {
        let mut app = setup_sync_app();
        let chunk = ChunkId::new(ChunkCoord::new(5, 6));
        prepare_resident_unit(&mut app, 5, 6);
        app.update();
        assert_eq!(app.world().resource::<UnitRenderIndex>().0.len(), 1);

        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .cancel(chunk);
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
    }

    #[test]
    fn sync_transform_matches_world_data_with_vertical_scale() {
        let mut app = setup_sync_app();
        let unit_id = prepare_resident_unit(&mut app, 7, 8);
        let record = app
            .world()
            .resource::<WorldData>()
            .get_unit(unit_id)
            .unwrap()
            .clone();
        let config = app.world().resource::<WorldConfig>().clone();
        let vertical_scale = 2.5;
        let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial::default());
        app.world_mut().insert_resource(TerrainRenderAssets {
            material,
            vertical_scale,
        });

        app.update();

        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        let transform = app
            .world()
            .entity(entity)
            .get::<Transform>()
            .expect("render entity transform");
        let expected = world_position_to_render_global(
            record.placement.position,
            config.chunk_layout(),
            vertical_scale,
        );
        assert_eq!(transform.translation, expected);
        assert_eq!(record.placement.position.local.0.y, 8.0);
        assert_eq!(transform.translation.y, 8.0 * vertical_scale);
        assert_eq!(transform.rotation, record.placement.rotation);
    }

    #[test]
    fn world_data_y_unscaled_after_sync() {
        let mut app = setup_sync_app();
        let unit_id = prepare_resident_unit(&mut app, 9, 10);
        let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial::default());
        app.world_mut().insert_resource(TerrainRenderAssets {
            material,
            vertical_scale: 4.0,
        });
        app.update();

        let y = app
            .world()
            .resource::<WorldData>()
            .get_unit(unit_id)
            .unwrap()
            .placement
            .position
            .local
            .0
            .y;
        assert_eq!(y, 8.0);
    }

    #[test]
    fn missing_definition_skips_spawn() {
        let mut app = setup_sync_app();
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 0, 0);
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    crate::world::UnitRecord::new(
                        id,
                        UnitDefinitionId::new("missing_unit"),
                        crate::world::UnitPlacement::new(
                            WorldPosition::new(
                                ChunkCoord::new(0, 0),
                                LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
                            ),
                            Quat::IDENTITY,
                        ),
                        UnitSource::Authored,
                        crate::world::UnitOwnership::neutral(),
                        10,
                    ),
                )
                .unwrap();
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.insert_resource(UnitSceneAssets::default());
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
    }

    #[test]
    fn missing_asset_skips_spawn_safely() {
        let mut app = setup_sync_app();
        let chunk = ChunkId::new(ChunkCoord::new(1, 1));
        let catalog = UnitCatalog::from_definitions(vec![UnitDefinition::new(
            UnitDefinitionId::new("ghost"),
            "Ghost",
            "Wild",
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "Common",
            4.0,
            0.5,
            40.0,
            crate::world::WeaponDefinitionId::new("weapon_fists"),
            true,
            UnitRenderKey::unset(),
        )])
        .unwrap();
        app.insert_resource(catalog.clone());
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 1, 1);
            create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("ghost"),
                WorldPosition::new(
                    ChunkCoord::new(1, 1),
                    LocalPosition::new(Vec3::ZERO),
                ),
                UnitSource::Authored,
            )
            .unwrap();
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.insert_resource(UnitSceneAssets::default());
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
    }

    #[test]
    fn death_pipeline_removal_despawns_render_entity() {
        let mut app = setup_sync_app();
        let unit_id = prepare_resident_unit(&mut app, 2, 2);
        app.update();
        assert_eq!(app.world().resource::<UnitRenderIndex>().0.len(), 1);

        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            world.damage_unit(unit_id, 999).unwrap();
            crate::world::step_unit_death_pipeline(&mut world, 1);
        }
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
    }

    #[test]
    fn removed_unit_record_despawns_entity() {
        let mut app = setup_sync_app();
        let unit_id = prepare_resident_unit(&mut app, 2, 2);
        app.update();
        assert_eq!(app.world().resource::<UnitRenderIndex>().0.len(), 1);

        app.world_mut()
            .resource_mut::<WorldData>()
            .remove_unit_by_id(unit_id);
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
    }
}
