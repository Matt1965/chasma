//! Sync building render entities with authoritative world data and terrain residency (ADR-079 B2, ADR-095 BA1).

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::terrain::TerrainRenderAssets;
use crate::terrain::residency::ChunkResidencyTracker;
use crate::world::{
    BuildingCatalog, BuildingId, WorldConfig, WorldData, building_anchor_render_transform,
};

use super::assets::{BuildingSceneAssets, lifecycle_render_key};
use super::components::BuildingRenderEntity;
use super::fallback::{BuildingFallbackAssets, BuildingFallbackReason};
use super::spawn::{
    BuildingRenderIndex, building_render_translation, despawn_building_render_entities,
    spawn_building_fallback_entity, spawn_building_scene_entity,
};

/// Systems that sync building render entities with world data.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct BuildingRuntimeSystems;

/// Test-only override for sync integration tests (not inserted in production).
#[derive(Resource, Default, Debug)]
pub struct BuildingSyncOverrides {
    pub treat_scenes_loaded: bool,
}

/// Collect building ids that should have render entities this frame.
pub(crate) fn visible_building_ids(
    world: &WorldData,
    residency: &ChunkResidencyTracker,
) -> HashSet<BuildingId> {
    let mut visible = HashSet::new();
    for (chunk_id, _) in world.iter() {
        if !residency.is_resident(chunk_id) {
            continue;
        }
        let Some(store) = world.buildings_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            visible.insert(record.id);
        }
    }
    visible
}

/// Keep derived building entities aligned with [`WorldData`] and terrain residency.
pub fn sync_building_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    catalog: Res<BuildingCatalog>,
    config: Res<WorldConfig>,
    residency: Res<ChunkResidencyTracker>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<BuildingSceneAssets>,
    mut fallback_assets: ResMut<BuildingFallbackAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut index: ResMut<BuildingRenderIndex>,
    existing: Query<(Entity, &BuildingRenderEntity)>,
    children: Query<&Children>,
    render_terrain: Option<Res<TerrainRenderAssets>>,
    overrides: Option<Res<BuildingSyncOverrides>>,
) {
    let vertical_scale = render_terrain
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let force_scenes_loaded = overrides
        .as_ref()
        .is_some_and(|value| value.treat_scenes_loaded);
    let should_render = visible_building_ids(&world, &residency);

    let stale: Vec<BuildingId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    despawn_building_render_entities(&mut commands, &mut index, stale);

    for (entity, marker) in &existing {
        if !should_render.contains(&marker.building_id) {
            continue;
        }
        let Some(record) = world.get_building(marker.building_id) else {
            commands.entity(entity).despawn();
            index.0.remove(&marker.building_id);
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            continue;
        };

        // Diagnostic fallbacks intentionally carry `active_render_key: None` even when a
        // desired render key exists (e.g. the GLB failed to load). Excluding them here
        // prevents an every-frame despawn/respawn churn that floods commands and logs.
        let desired_key = lifecycle_render_key(definition, record.lifecycle_state);
        if !marker.uses_diagnostic_fallback && marker.active_render_key != desired_key {
            commands.entity(entity).despawn();
            index.0.remove(&marker.building_id);
            continue;
        }

        let translation = building_render_translation(&record, &config, vertical_scale);
        if marker.uses_diagnostic_fallback {
            let mesh_size = super::placeholder::placeholder_mesh_size(definition);
            let mut fallback_translation = translation;
            fallback_translation.y += mesh_size.y * 0.5;
            commands.entity(entity).insert(Transform {
                translation: fallback_translation,
                rotation: record.placement.rotation,
                scale: Vec3::ONE,
            });
        } else {
            let layout = config.chunk_layout();
            if crate::world::building_uses_model_child(definition) {
                let anchor = building_anchor_render_transform(
                    definition,
                    &record.placement,
                    layout,
                    vertical_scale,
                );
                commands.entity(entity).insert(anchor);
                // Instance uniform scale lives on the model child; keep it in sync so
                // dev scale edits persist after the object is deselected (the gizmo
                // preview otherwise masks a stale child transform).
                let correction = crate::world::building_model_child_local_transform(
                    definition,
                    record.placement.uniform_scale_f32(),
                );
                if let Ok(child_entities) = children.get(entity) {
                    for child in child_entities.iter() {
                        commands.entity(child).insert(correction);
                    }
                }
            } else {
                let transform = crate::world::building_model_render_transform(
                    definition,
                    &record.placement,
                    layout,
                    vertical_scale,
                );
                commands.entity(entity).insert(transform);
            }
        }

        if marker.lifecycle_state != record.lifecycle_state {
            commands.entity(entity).insert(BuildingRenderEntity {
                building_id: marker.building_id,
                chunk_id: marker.chunk_id,
                lifecycle_state: record.lifecycle_state,
                active_render_key: marker.active_render_key.clone(),
                uses_diagnostic_fallback: marker.uses_diagnostic_fallback,
            });
            if marker.uses_diagnostic_fallback {
                commands
                    .entity(entity)
                    .remove::<BuildingLifecycleTintApplied>();
            } else {
                commands
                    .entity(entity)
                    .remove::<super::components::BuildingLifecycleTintApplied>();
            }
        }
    }

    for id in should_render {
        if index.0.contains_key(&id) {
            continue;
        }

        let Some(chunk_id) = world.building_chunk(id) else {
            continue;
        };
        let Some(record) = world.get_building(id) else {
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            warn!(
                "building {} references missing definition `{}`",
                record.id.raw(),
                record.definition_id.as_str()
            );
            let placeholder = missing_definition_placeholder(record);
            let entity = spawn_building_fallback_entity(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut fallback_assets,
                record,
                &placeholder,
                chunk_id,
                &config,
                vertical_scale,
                BuildingFallbackReason::MissingDefinition,
            );
            index.0.insert(id, entity);
            continue;
        };

        let Some(render_key) = lifecycle_render_key(definition, record.lifecycle_state) else {
            scene_assets.log_missing_once(&record.definition_id.as_str());
            let entity = spawn_building_fallback_entity(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut fallback_assets,
                record,
                definition,
                chunk_id,
                &config,
                vertical_scale,
                BuildingFallbackReason::MissingRenderKey,
            );
            index.0.insert(id, entity);
            continue;
        };

        let Some(scene) = scene_assets.ensure_scene(&render_key, &asset_server) else {
            scene_assets.log_missing_once(&render_key);
            let entity = spawn_building_fallback_entity(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut fallback_assets,
                record,
                definition,
                chunk_id,
                &config,
                vertical_scale,
                BuildingFallbackReason::MissingRenderKey,
            );
            index.0.insert(id, entity);
            continue;
        };

        let load_state = asset_server.get_load_state(&scene);
        if !force_scenes_loaded {
            match load_state {
                Some(LoadState::Loaded) => {}
                Some(LoadState::Failed(_)) => {
                    scene_assets.log_failed_once(&render_key);
                    let entity = spawn_building_fallback_entity(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &mut fallback_assets,
                        record,
                        definition,
                        chunk_id,
                        &config,
                        vertical_scale,
                        BuildingFallbackReason::AssetLoadFailed,
                    );
                    index.0.insert(id, entity);
                    continue;
                }
                _ => continue,
            }
        }

        let entity = spawn_building_scene_entity(
            &mut commands,
            record,
            definition,
            chunk_id,
            scene,
            render_key,
            &config,
            vertical_scale,
        );
        index.0.insert(id, entity);
    }
}

use super::components::BuildingLifecycleTintApplied;

fn missing_definition_placeholder(
    record: &crate::world::BuildingRecord,
) -> crate::world::BuildingDefinition {
    use crate::world::{
        BuildingCategoryId, BuildingDefinition, BuildingDefinitionId, BuildingRenderKey,
        FootprintSpec,
    };
    BuildingDefinition::new(
        record.definition_id.clone(),
        "Missing Definition",
        BuildingCategoryId::new("unknown"),
        BuildingRenderKey::unset(),
        BuildingRenderKey::unset(),
        record.vitals.max_hp.max(1),
        0.0,
        FootprintSpec::Rectangle {
            width_meters: 2.0,
            depth_meters: 2.0,
        },
        0.0,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::super::components::BuildingDiagnosticFallback;
    use super::*;
    use crate::buildings::assets::BuildingSceneAssets;
    use crate::terrain::world_position_to_render_global;
    use crate::world::{
        BuildingCatalog, BuildingCategoryCatalog, BuildingDefinitionId, BuildingId,
        BuildingOwnership, BuildingPlacement, BuildingRecord, BuildingRenderKey, BuildingSource,
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, WorldConfig,
        WorldData, WorldPosition, starter_building_definitions,
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

    fn insert_authored_building(world: &mut WorldData, x: i32, z: i32) -> BuildingId {
        let id = world.allocate_building_id();
        let record = BuildingRecord::new(
            id,
            BuildingDefinitionId::new("hut"),
            BuildingPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(x, z),
                    LocalPosition::new(Vec3::new(20.0, 12.0, 30.0)),
                ),
                Quat::from_rotation_y(0.5),
            ),
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            250,
            BuildingSource::Authored,
        );
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        world.insert_building(chunk, record).unwrap();
        id
    }

    fn setup_sync_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<BuildingCatalog>();
        app.init_resource::<ChunkResidencyTracker>();
        app.init_resource::<BuildingRenderIndex>();
        app.init_resource::<BuildingFallbackAssets>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<Assets<Scene>>();
        app.insert_resource(BuildingSyncOverrides {
            treat_scenes_loaded: true,
        });
        app.add_systems(Update, sync_building_render_entities);
        app
    }

    fn prepare_resident_building(app: &mut App, x: i32, z: i32) -> BuildingId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(World::new()))
        };
        app.insert_resource(BuildingSceneAssets::from_test_scenes(HashMap::from([(
            "hut".to_string(),
            scene,
        )])));
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, x, z);
            insert_authored_building(&mut world, x, z);
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.world()
            .resource::<WorldData>()
            .sorted_building_ids()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn visible_ids_require_resident_terrain() {
        let mut world = WorldData::new(layout());
        insert_terrain(&mut world, 0, 0);
        let id = insert_authored_building(&mut world, 0, 0);

        let mut residency = ChunkResidencyTracker::default();
        assert!(visible_building_ids(&world, &residency).is_empty());

        residency.mark_resident(ChunkId::new(ChunkCoord::new(0, 0)));
        assert_eq!(
            visible_building_ids(&world, &residency),
            HashSet::from([id])
        );
    }

    #[test]
    fn sync_spawns_scene_root_for_valid_asset() {
        let mut app = setup_sync_app();
        let building_id = prepare_resident_building(&mut app, 1, 2);
        app.update();

        let index = app.world().resource::<BuildingRenderIndex>();
        assert_eq!(index.0.len(), 1);
        let entity = index.0[&building_id];
        assert!(
            app.world()
                .entity(entity)
                .get::<super::super::components::BuildingSceneRoot>()
                .is_some()
        );
        assert!(
            app.world()
                .entity(entity)
                .get::<BuildingDiagnosticFallback>()
                .is_none()
        );
    }

    #[test]
    fn sync_does_not_duplicate_across_ticks() {
        let mut app = setup_sync_app();
        let building_id = prepare_resident_building(&mut app, 3, 4);
        app.update();
        app.update();

        let index = app.world().resource::<BuildingRenderIndex>();
        assert_eq!(index.0.len(), 1);
        assert!(index.0.contains_key(&building_id));
    }

    #[test]
    fn sync_despawns_when_chunk_not_resident() {
        let mut app = setup_sync_app();
        let chunk = ChunkId::new(ChunkCoord::new(5, 6));
        prepare_resident_building(&mut app, 5, 6);
        app.update();
        assert_eq!(app.world().resource::<BuildingRenderIndex>().0.len(), 1);

        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .cancel(chunk);
        app.update();
        assert!(app.world().resource::<BuildingRenderIndex>().0.is_empty());
    }

    #[test]
    fn sync_transform_uses_ground_anchor_without_cuboid_offset() {
        let mut app = setup_sync_app();
        let building_id = prepare_resident_building(&mut app, 7, 8);
        let record = app
            .world()
            .resource::<WorldData>()
            .get_building(building_id)
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

        let entity = app.world().resource::<BuildingRenderIndex>().0[&building_id];
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
    }

    #[test]
    fn missing_render_key_uses_diagnostic_fallback() {
        let mut app = setup_sync_app();
        app.insert_resource(BuildingSceneAssets::default());
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let categories = BuildingCategoryCatalog::default();
        let mut defs = starter_building_definitions();
        for def in &mut defs {
            if def.id.as_str() == "hut" {
                def.render_key = BuildingRenderKey::unset();
            }
        }
        let catalog = BuildingCatalog::from_definitions(defs, &categories).unwrap();
        app.insert_resource(catalog);
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 0, 0);
            insert_authored_building(&mut world, 0, 0);
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.update();
        let entity = app
            .world()
            .resource::<BuildingRenderIndex>()
            .0
            .values()
            .next()
            .copied()
            .unwrap();
        assert!(
            app.world()
                .entity(entity)
                .get::<BuildingDiagnosticFallback>()
                .is_some()
        );
    }

    #[test]
    fn runtime_recreation_after_despawn_respawns_same_id() {
        let mut app = setup_sync_app();
        let chunk = ChunkId::new(ChunkCoord::new(1, 1));
        let building_id = prepare_resident_building(&mut app, 1, 1);
        app.update();
        let first_entity = app.world().resource::<BuildingRenderIndex>().0[&building_id];

        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .cancel(chunk);
        app.update();
        assert!(app.world().resource::<BuildingRenderIndex>().0.is_empty());

        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.update();
        let second_entity = app.world().resource::<BuildingRenderIndex>().0[&building_id];
        assert_ne!(first_entity, second_entity);
    }
}
