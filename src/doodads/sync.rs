//! Sync doodad render entities with authoritative world data and terrain residency (ADR-023).
//!
//! [`WorldData`] stores authoritative placement Y from terrain sampling (ADR-022).
//! Render transforms multiply Y by [`TerrainRenderAssets::vertical_scale`] so props
//! align with the visible terrain mesh (ADR-010). World records are never modified.

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{DoodadCatalog, DoodadId, WorldConfig, WorldData, doodad_final_render_scale};

use super::assets::DoodadSceneAssets;
use super::components::DoodadRenderEntity;
use super::spawn::{DoodadRenderIndex, despawn_doodad_render_entities, spawn_doodad_render_entity};

/// Systems that sync doodad render entities with world data.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DoodadRuntimeSystems;

/// Test-only override for sync integration tests (not inserted in production).
#[derive(Resource, Default, Debug)]
pub struct DoodadSyncOverrides {
    pub treat_scenes_loaded: bool,
}

/// Collect doodad ids that should have render entities this frame.
pub(crate) fn visible_doodad_ids(
    world: &WorldData,
    residency: &ChunkResidencyTracker,
) -> HashSet<DoodadId> {
    let mut visible = HashSet::new();
    for (chunk_id, _) in world.iter() {
        if !residency.is_resident(chunk_id) {
            continue;
        }
        let Some(store) = world.doodads_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            visible.insert(record.id);
        }
    }
    visible
}

/// Keep derived doodad entities aligned with [`WorldData`] and terrain residency.
pub fn sync_doodad_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    catalog: Res<DoodadCatalog>,
    config: Res<WorldConfig>,
    residency: Res<ChunkResidencyTracker>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<DoodadSceneAssets>,
    mut index: ResMut<DoodadRenderIndex>,
    existing: Query<(Entity, &DoodadRenderEntity)>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    overrides: Option<Res<DoodadSyncOverrides>>,
) {
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let force_scenes_loaded = overrides
        .as_ref()
        .is_some_and(|value| value.treat_scenes_loaded);
    let should_render = visible_doodad_ids(&world, &residency);

    let stale: Vec<DoodadId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    despawn_doodad_render_entities(&mut commands, &mut index, stale);

    for (entity, marker) in &existing {
        if !should_render.contains(&marker.doodad_id) {
            continue;
        }
        let Some(record) = world.get_doodad(marker.doodad_id) else {
            commands.entity(entity).despawn();
            index.0.remove(&marker.doodad_id);
            continue;
        };
        let layout = config.chunk_layout();
        let translation =
            world_position_to_render_global(record.placement.position, layout, vertical_scale);
        let scale = catalog
            .get(&record.definition_id)
            .map(|definition| doodad_final_render_scale(definition, record.placement.scale_vec3()))
            .unwrap_or_else(|| record.placement.scale_vec3());
        commands.entity(entity).insert(Transform {
            translation,
            rotation: record.placement.rotation_quat(),
            scale,
        });
    }

    for id in should_render {
        if index.0.contains_key(&id) {
            continue;
        }

        let Some(chunk_id) = world.doodad_chunk(id) else {
            continue;
        };
        let Some(record) = world.get_doodad(id) else {
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            warn!(
                "doodad {} references missing definition `{}`",
                record.id.raw(),
                record.definition_id.as_str()
            );
            continue;
        };
        let Some(scene) =
            scene_assets.ensure_scene(&definition.id, &definition.render_key, &asset_server)
        else {
            if let Some(key) = definition.render_key.0.as_deref() {
                scene_assets.log_missing_once(key);
            }
            continue;
        };
        if !force_scenes_loaded && !scene_is_loaded(&asset_server, &scene) {
            continue;
        }

        let entity = spawn_doodad_render_entity(
            &mut commands,
            record,
            definition,
            chunk_id,
            scene,
            &config,
            vertical_scale,
        );
        index.0.insert(id, entity);
    }
}

fn scene_is_loaded(asset_server: &AssetServer, scene: &Handle<Scene>) -> bool {
    matches!(asset_server.get_load_state(scene), Some(LoadState::Loaded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
        DoodadDefinitionId, DoodadId, DoodadPlacementOverrides, DoodadSource, FootprintCatalog,
        Heightfield, LocalPosition, WorldConfig, WorldData, WorldPosition, create_doodad,
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
        let samples = vec![12.0; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn insert_authored_doodad(world: &mut WorldData, catalog: &DoodadCatalog, x: i32, z: i32) {
        create_doodad(
            catalog,
            world,
            &DoodadDefinitionId::new("tree_oak"),
            WorldPosition::new(
                ChunkCoord::new(x, z),
                LocalPosition::new(Vec3::new(20.0, 12.0, 30.0)),
            ),
            DoodadSource::Authored,
            DoodadPlacementOverrides {
                rotation: Some(Quat::from_rotation_y(1.25)),
                scale: Some(Vec3::splat(1.1)),
            },
            None,
        )
        .unwrap();
    }

    fn setup_sync_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<DoodadCatalog>();
        app.init_resource::<ChunkResidencyTracker>();
        app.init_resource::<DoodadRenderIndex>();
        app.init_resource::<Assets<Scene>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.insert_resource(DoodadSyncOverrides {
            treat_scenes_loaded: true,
        });
        app.add_systems(Update, sync_doodad_render_entities);
        app
    }

    fn prepare_resident_doodad(app: &mut App, x: i32, z: i32) -> DoodadId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        let catalog = app.world().resource::<DoodadCatalog>().clone();
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(World::new()))
        };
        app.insert_resource(DoodadSceneAssets::from_test_scenes(HashMap::from([(
            DoodadDefinitionId::new("tree_oak"),
            scene,
        )])));
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, x, z);
            insert_authored_doodad(&mut world, &catalog, x, z);
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.world()
            .resource::<WorldData>()
            .doodads_in_chunk(chunk)
            .unwrap()
            .records()[0]
            .id
    }

    #[test]
    fn visible_ids_require_resident_terrain() {
        let mut world = WorldData::new(layout());
        let catalog = DoodadCatalog::default();
        insert_terrain(&mut world, 0, 0);
        insert_authored_doodad(&mut world, &catalog, 0, 0);
        let id = world
            .doodads_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap()
            .records()[0]
            .id;

        let mut residency = ChunkResidencyTracker::default();
        assert!(visible_doodad_ids(&world, &residency).is_empty());

        residency.mark_resident(ChunkId::new(ChunkCoord::new(0, 0)));
        assert_eq!(visible_doodad_ids(&world, &residency), HashSet::from([id]));
    }

    #[test]
    fn sync_spawns_render_entity_for_resident_record() {
        let mut app = setup_sync_app();
        let doodad_id = prepare_resident_doodad(&mut app, 1, 2);
        app.update();

        let index = app.world().resource::<DoodadRenderIndex>();
        assert_eq!(index.0.len(), 1);
        assert!(index.0.contains_key(&doodad_id));
    }

    #[test]
    fn sync_does_not_duplicate_across_ticks() {
        let mut app = setup_sync_app();
        let doodad_id = prepare_resident_doodad(&mut app, 3, 4);
        app.update();
        app.update();

        let index = app.world().resource::<DoodadRenderIndex>();
        assert_eq!(index.0.len(), 1);
        assert!(index.0.contains_key(&doodad_id));
    }

    #[test]
    fn sync_despawns_when_chunk_not_resident() {
        let mut app = setup_sync_app();
        let chunk = ChunkId::new(ChunkCoord::new(5, 6));
        prepare_resident_doodad(&mut app, 5, 6);
        app.update();
        assert_eq!(app.world().resource::<DoodadRenderIndex>().0.len(), 1);

        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .cancel(chunk);
        app.update();
        assert!(app.world().resource::<DoodadRenderIndex>().0.is_empty());
    }

    #[test]
    fn sync_transform_matches_world_data_with_vertical_scale() {
        let mut app = setup_sync_app();
        let doodad_id = prepare_resident_doodad(&mut app, 7, 8);
        let record = app
            .world()
            .resource::<WorldData>()
            .get_doodad(doodad_id)
            .unwrap()
            .clone();
        let config = app.world().resource::<WorldConfig>().clone();
        let catalog = app.world().resource::<DoodadCatalog>().clone();
        let vertical_scale = 2.5;
        let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial::default());
        app.world_mut().insert_resource(TerrainRenderAssets {
            material,
            vertical_scale,
        });

        app.update();

        let entity = app.world().resource::<DoodadRenderIndex>().0[&doodad_id];
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
        assert_eq!(record.placement.position.local.0.y, 12.0);
        assert_eq!(transform.translation.y, 12.0 * vertical_scale);
        assert_eq!(transform.rotation, record.placement.rotation_quat());
        let expected_scale = catalog
            .get(&record.definition_id)
            .map(|definition| {
                crate::world::doodad_final_render_scale(definition, record.placement.scale_vec3())
            })
            .unwrap_or_else(|| record.placement.scale_vec3());
        assert_eq!(transform.scale, expected_scale);
    }

    #[test]
    fn world_data_y_unscaled_after_sync() {
        let mut app = setup_sync_app();
        let doodad_id = prepare_resident_doodad(&mut app, 9, 10);
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
            .get_doodad(doodad_id)
            .unwrap()
            .placement
            .position
            .local
            .0
            .y;
        assert_eq!(y, 12.0);
    }
}
