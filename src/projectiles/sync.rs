//! Sync projectile render entities with authoritative world data (ADR-060 C7).

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{world_position_to_render_global, TerrainRenderAssets};
use crate::world::{ChunkId, ProjectileId, WeaponCatalog, WorldConfig, WorldData};

use super::assets::ProjectileSceneAssets;
use super::components::{ProjectileRenderEntity, ProjectileSceneRoot};
use super::spawn::ProjectileRenderIndex;

/// Systems that sync projectile render entities with world data.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ProjectileRuntimeSystems;

/// Test-only override for sync integration tests.
#[derive(Resource, Default, Debug)]
pub struct ProjectileSyncOverrides {
    pub treat_scenes_loaded: bool,
}

pub(crate) fn visible_projectile_ids(
    world: &WorldData,
    residency: &ChunkResidencyTracker,
) -> HashSet<ProjectileId> {
    world
        .sorted_projectile_ids()
        .into_iter()
        .filter(|id| {
            world
                .get_projectile(*id)
                .map(|record| residency.is_resident(ChunkId::new(record.position.chunk)))
                .unwrap_or(false)
        })
        .collect()
}

pub fn sync_projectile_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    weapons: Res<WeaponCatalog>,
    config: Res<WorldConfig>,
    residency: Res<ChunkResidencyTracker>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<ProjectileSceneAssets>,
    mut index: ResMut<ProjectileRenderIndex>,
    existing: Query<(Entity, &ProjectileRenderEntity)>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    overrides: Option<Res<ProjectileSyncOverrides>>,
) {
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let force_scenes_loaded = overrides
        .as_ref()
        .is_some_and(|value| value.treat_scenes_loaded);
    let should_render = visible_projectile_ids(&world, &residency);

    let stale: Vec<ProjectileId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    for id in stale {
        if let Some(entity) = index.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }

    let layout = config.chunk_layout();
    for (entity, marker) in &existing {
        let Some(record) = world.get_projectile(marker.projectile_id) else {
            commands.entity(entity).despawn();
            index.0.remove(&marker.projectile_id);
            continue;
        };
        if !should_render.contains(&marker.projectile_id) {
            continue;
        }
        let translation =
            world_position_to_render_global(record.position, layout, vertical_scale);
        commands.entity(entity).insert(Transform {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(1.0, vertical_scale, 1.0),
        });
    }

    for id in should_render {
        if index.0.contains_key(&id) {
            continue;
        }
        let Some(record) = world.get_projectile(id) else {
            continue;
        };
        let Some(weapon) = weapons.get(&record.weapon_id) else {
            warn!(
                "projectile {} references missing weapon `{}`",
                record.id.raw(),
                record.weapon_id.as_str()
            );
            continue;
        };
        let Some(projectile_key) = weapon.projectile_key.as_deref() else {
            continue;
        };
        let Some(scene) = scene_assets.ensure_scene(projectile_key, &asset_server) else {
            scene_assets.log_missing_once(projectile_key);
            continue;
        };
        if !force_scenes_loaded && !scene_is_loaded(&asset_server, &scene) {
            continue;
        }

        let translation =
            world_position_to_render_global(record.position, layout, vertical_scale);
        let entity = commands
            .spawn((
                ProjectileRenderEntity {
                    projectile_id: id,
                },
                ProjectileSceneRoot,
                SceneRoot(scene),
                Transform {
                    translation,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::new(1.0, vertical_scale, 1.0),
                },
                Visibility::default(),
            ))
            .id();
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
    use bevy::prelude::{App, AssetPlugin, Assets, MinimalPlugins, Scene, StandardMaterial, Vec3, World as BevyWorld};
    use std::collections::HashMap;
    use crate::terrain::residency::ChunkResidencyTracker;
    use crate::terrain::TerrainRenderAssets;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DamageType, Heightfield, HitMode,
        LocalPosition, ProjectileId, ProjectileLaunchSnapshot, ProjectileRecord, TargetFilter,
        UnitId, WeaponCatalog, WeaponDefinition, WeaponDefinitionId, WorldConfig, WorldData,
        WorldPosition,
    };

    fn setup_sync_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<ChunkResidencyTracker>();
        app.init_resource::<ProjectileRenderIndex>();
        app.init_resource::<ProjectileSceneAssets>();
        app.init_resource::<Assets<Scene>>();
        app.init_resource::<Assets<StandardMaterial>>();
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(BevyWorld::new()))
        };
        app.insert_resource(ProjectileSceneAssets::from_test_scenes(HashMap::from([(
            "arrow".to_string(),
            scene,
        )])));
        app.insert_resource(WeaponCatalog::from_definitions(vec![WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_bow"),
            "Bow",
            "Test",
            5.0,
            DamageType::Piercing,
            10.0,
            1.0,
            0.1,
            0.1,
            HitMode::Projectile,
            Some("arrow".to_string()),
            20.0,
            "attack_bow",
            vec![TargetFilter::Enemies],
            None,
            true,
        )])
        .unwrap());
        app.add_systems(Update, sync_projectile_render_entities);
        app
    }

    fn insert_terrain(world: &mut WorldData, cx: i32, cz: i32) {
        let chunk = ChunkId::new(ChunkCoord::new(cx, cz));
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(chunk, ChunkData::new(heightfield, Vec::new()));
    }

    #[test]
    fn sync_spawns_projectile_visual_when_override_active() {
        let mut app = setup_sync_app();
        app.insert_resource(ProjectileSyncOverrides {
            treat_scenes_loaded: true,
        });
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 0, 0);
            world.insert_projectile(ProjectileRecord::new_in_flight(
                ProjectileId::new(1),
                UnitId::new(10),
                UnitId::new(20),
                WeaponDefinitionId::new("weapon_bow"),
                5.0,
                DamageType::Piercing,
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(5.0, 0.0, 5.0)),
                ),
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(8.0, 0.0, 5.0)),
                ),
                20.0,
                ProjectileLaunchSnapshot::render_test_placeholder(UnitId::new(10)),
            ));
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.update();
        assert_eq!(app.world().resource::<ProjectileRenderIndex>().0.len(), 1);
    }

    #[test]
    fn sync_despawns_projectile_visual_after_removal() {
        let mut app = setup_sync_app();
        app.insert_resource(ProjectileSyncOverrides {
            treat_scenes_loaded: true,
        });
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let projectile_id = ProjectileId::new(1);
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 0, 0);
            world.insert_projectile(ProjectileRecord::new_in_flight(
                projectile_id,
                UnitId::new(10),
                UnitId::new(20),
                WeaponDefinitionId::new("weapon_bow"),
                5.0,
                DamageType::Piercing,
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(5.0, 0.0, 5.0)),
                ),
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(8.0, 0.0, 5.0)),
                ),
                20.0,
                ProjectileLaunchSnapshot::render_test_placeholder(UnitId::new(10)),
            ));
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.update();
        assert_eq!(app.world().resource::<ProjectileRenderIndex>().0.len(), 1);

        app.world_mut().resource_mut::<WorldData>().remove_projectile(projectile_id);
        app.update();
        assert!(app.world().resource::<ProjectileRenderIndex>().0.is_empty());
    }

    #[test]
    fn render_entity_does_not_mutate_authoritative_projectile_position() {
        let mut app = setup_sync_app();
        app.insert_resource(ProjectileSyncOverrides {
            treat_scenes_loaded: true,
        });
        let material = {
            let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
            materials.add(StandardMaterial::default())
        };
        app.insert_resource(TerrainRenderAssets {
            material,
            vertical_scale: 2.0,
        });
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let projectile_id = ProjectileId::new(1);
        let authoritative = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(5.0, 4.0, 5.0)),
        );
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, 0, 0);
            world.insert_projectile(ProjectileRecord::new_in_flight(
                projectile_id,
                UnitId::new(10),
                UnitId::new(20),
                WeaponDefinitionId::new("weapon_bow"),
                5.0,
                DamageType::Piercing,
                authoritative,
                authoritative,
                20.0,
                ProjectileLaunchSnapshot::render_test_placeholder(UnitId::new(10)),
            ));
        }
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.update();
        assert_eq!(
            app.world()
                .resource::<WorldData>()
                .get_projectile(projectile_id)
                .unwrap()
                .position,
            authoritative
        );
    }
}
