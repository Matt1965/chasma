//! Sync unit render entities with authoritative world data and terrain residency (ADR-028).
//!
//! [`WorldData`] stores authoritative placement Y in world units. Render transforms
//! multiply Y by [`TerrainRenderAssets::vertical_scale`] so units align with the
//! visible terrain mesh (ADR-010). World records are never modified.

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::terrain::TerrainRenderAssets;
use crate::terrain::residency::ChunkResidencyTracker;
use crate::world::{UnitCatalog, UnitId, WorldConfig, WorldData};

use super::assets::UnitSceneAssets;
use super::components::UnitRenderEntity;
use super::spawn::{
    UnitRenderIndex, despawn_unit_render_entities, spawn_unit_render_entity,
    unit_render_translation,
};

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
    existing: Query<(Entity, &UnitRenderEntity, &Transform)>,
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
    for id in stale {
        if world.get_unit(id).is_some() {
            despawn_unit_render_entities(&mut commands, &mut index, [id]);
        }
    }

    for (entity, marker, transform) in &existing {
        if !should_render.contains(&marker.unit_id) {
            continue;
        }
        let Some(record) = world.get_unit(marker.unit_id) else {
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            continue;
        };
        let render_scale = crate::world::unit_visual_scale(definition);
        let layout = config.chunk_layout();
        let translation = unit_render_translation(&world, record, layout, vertical_scale);
        commands.entity(entity).insert(Transform {
            translation,
            rotation: transform.rotation,
            scale: render_scale,
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
            &world,
            record,
            definition,
            scene,
            &config,
            vertical_scale,
            crate::world::unit_visual_scale(definition),
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
    use crate::units::animation::{
        DeathPresentation, UnitAnimationSettings, begin_death_presentations,
    };
    use crate::units::{UnitSceneAssets, UnitSyncOverrides};
    use crate::world::{AnimationProfileCatalog, AnimationProfileId};
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinition, UnitDefinitionId, UnitId, UnitRenderKey, UnitSource, WorldConfig,
        WorldData, WorldPosition, create_unit,
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

    fn insert_authored_unit(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        x: i32,
        z: i32,
    ) -> UnitId {
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

    fn catalog_with_wolf_animation_profile() -> UnitCatalog {
        let definitions: Vec<UnitDefinition> = UnitCatalog::default()
            .definitions()
            .iter()
            .map(|definition| {
                let mut definition = definition.clone();
                if definition.id.as_str() == "wolf" {
                    definition.animation_profile_id = Some(AnimationProfileId::new("quadruped"));
                }
                definition
            })
            .collect();
        UnitCatalog::from_definitions(definitions).unwrap()
    }

    fn setup_sync_with_death_presentation_app() -> App {
        let mut app = setup_sync_app();
        app.insert_resource(catalog_with_wolf_animation_profile());
        app.init_resource::<AnimationProfileCatalog>();
        app.init_resource::<UnitAnimationSettings>();
        app.add_systems(
            Update,
            begin_death_presentations.after(sync_unit_render_entities),
        );
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
        assert_eq!(
            transform.rotation,
            crate::world::unit_visual_rotation(
                app.world()
                    .resource::<UnitCatalog>()
                    .get(&record.definition_id)
                    .expect("definition"),
                record.placement.rotation,
            )
        );
    }

    #[test]
    fn sync_applies_definition_rotation_correction() {
        let mut app = setup_sync_app();
        let mut definition = UnitDefinition::new(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            2,
            5,
            5,
            4,
            6,
            3,
            7,
            2,
            3,
            26.5,
            "Elite",
            4.0,
            0.6,
            40.0,
            crate::world::WeaponDefinitionId::new("weapon_wolf_bite"),
            true,
            UnitRenderKey::reserved("wolf"),
        );
        definition.asset_sizing.rotation_correction =
            crate::world::QuantizedOrientation::from_degrees(90.0, 0.0, 0.0).unwrap();
        let catalog = UnitCatalog::from_definitions(vec![definition.clone()]).unwrap();
        app.insert_resource(catalog);
        let unit_id = prepare_resident_unit(&mut app, 3, 4);
        let record = app
            .world()
            .resource::<WorldData>()
            .get_unit(unit_id)
            .unwrap()
            .clone();
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        let transform = app.world().entity(entity).get::<Transform>().unwrap();
        assert_eq!(
            transform.rotation,
            crate::world::unit_visual_rotation(&definition, record.placement.rotation)
        );
        assert_ne!(transform.rotation, record.placement.rotation);
    }

    #[test]
    fn sync_applies_definition_render_scale() {
        let mut app = setup_sync_app();
        let mut definition = UnitDefinition::new(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            2,
            5,
            5,
            4,
            6,
            3,
            7,
            2,
            3,
            26.5,
            "Elite",
            4.5,
            0.6,
            40.0,
            crate::world::WeaponDefinitionId::new("weapon_wolf_bite"),
            true,
            UnitRenderKey::reserved("wolf"),
        );
        definition.asset_sizing.explicit_baseline_scale =
            Some(crate::world::AuthoringScale::from_uniform_f32(2.15).unwrap());
        let catalog = UnitCatalog::from_definitions(vec![definition]).unwrap();
        app.insert_resource(catalog);
        let unit_id = prepare_resident_unit(&mut app, 9, 10);
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        let transform = app.world().entity(entity).get::<Transform>().unwrap();
        assert_eq!(transform.scale, Vec3::splat(2.15));
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

    /// IN-11c: interior units must render on the building's visible floor.
    ///
    /// Terrain relief is exaggerated by a large factor in the dev world. Multiplying an
    /// interior floor offset by that factor threw units thousands of units into the sky,
    /// which read in game as the unit vanishing on entry.
    mod interior_presentation {
        use super::*;
        use crate::world::{
            Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingId, BuildingOwnership,
            DoodadCatalog, FootprintCatalog, OccupancyCatalogs, SpaceId, SpaceRecord,
            place_player_building,
        };

        const EXAGGERATION: f32 = 18_336.0;
        const FLOOR_OFFSET_METERS: f32 = 1.104;

        fn place_hut(app: &mut App) -> (BuildingId, f32) {
            let building_catalog = BuildingCatalog::default();
            let doodad_catalog = DoodadCatalog::default();
            let footprint = FootprintCatalog::default();
            let mut world = app.world_mut().resource_mut::<WorldData>();
            let building_id = place_player_building(
                &building_catalog,
                &mut world,
                &BuildingDefinitionId::new("hut"),
                WorldPosition::new(
                    ChunkCoord::new(1, 2),
                    LocalPosition::new(Vec3::new(60.0, 8.0, 60.0)),
                ),
                Quat::IDENTITY,
                BuildingOwnership::with_affiliation(Affiliation::Player),
                OccupancyCatalogs {
                    building: &building_catalog,
                    doodad: &doodad_catalog,
                    footprint: &footprint,
                },
            )
            .expect("place hut")
            .id;
            let anchor_y = world
                .get_building(building_id)
                .expect("building")
                .placement
                .position
                .to_global(world.layout())
                .y;
            (building_id, anchor_y)
        }

        fn add_region(app: &mut App, building_id: BuildingId, floor_y: f32) -> SpaceId {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            let registry = world.space_registry_mut();
            let space_id = registry.allocate_space_id();
            registry.insert_space(SpaceRecord {
                id: space_id,
                owning_building_id: Some(building_id),
                display_floor_label: "Ground".into(),
                // One visibility group shared by every region of this building.
                visibility_group_id: 1,
                reference_elevation: FLOOR_OFFSET_METERS,
                floor_y_global: floor_y,
                room_tag: None,
                enabled: true,
                walkable: true,
            });
            space_id
        }

        fn scaled_app() -> App {
            let mut app = setup_sync_app();
            let material = {
                let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
                materials.add(StandardMaterial::default())
            };
            app.world_mut().insert_resource(TerrainRenderAssets {
                material,
                vertical_scale: EXAGGERATION,
            });
            app
        }

        fn render_y(app: &App, unit_id: UnitId) -> f32 {
            let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
            app.world()
                .entity(entity)
                .get::<Transform>()
                .expect("transform")
                .translation
                .y
        }

        #[test]
        fn interior_unit_renders_on_the_visible_floor_not_the_exaggerated_one() {
            let mut app = scaled_app();
            let unit_id = prepare_resident_unit(&mut app, 1, 2);
            let (building_id, anchor_y) = place_hut(&mut app);
            let floor_y = anchor_y + FLOOR_OFFSET_METERS;
            let space_id = add_region(&mut app, building_id, floor_y);
            {
                let mut world = app.world_mut().resource_mut::<WorldData>();
                world.set_unit_current_space(unit_id, space_id).unwrap();
                let position = world.get_unit(unit_id).unwrap().placement.position;
                let mut interior = position;
                interior.local.0.y = floor_y;
                world.relocate_unit(unit_id, interior).unwrap();
            }
            app.update();

            let expected = anchor_y * EXAGGERATION + FLOOR_OFFSET_METERS;
            assert!(
                (render_y(&app, unit_id) - expected).abs() < 0.01,
                "interior unit rendered at {} but the visible floor is at {expected}",
                render_y(&app, unit_id)
            );
            assert!(
                (render_y(&app, unit_id) - floor_y * EXAGGERATION).abs() > 1000.0,
                "guard: the terrain-exaggerated mapping is what threw the unit off-world"
            );
        }

        #[test]
        fn two_regions_of_one_building_share_the_same_render_base() {
            let mut app = scaled_app();
            let unit_id = prepare_resident_unit(&mut app, 1, 2);
            let (building_id, anchor_y) = place_hut(&mut app);
            let ground = add_region(&mut app, building_id, anchor_y + FLOOR_OFFSET_METERS);
            let upper = add_region(&mut app, building_id, anchor_y + FLOOR_OFFSET_METERS + 3.0);

            let mut sample = |space_id, floor_y: f32| {
                {
                    let mut world = app.world_mut().resource_mut::<WorldData>();
                    world.set_unit_current_space(unit_id, space_id).unwrap();
                    let mut position = world.get_unit(unit_id).unwrap().placement.position;
                    position.local.0.y = floor_y;
                    world.relocate_unit(unit_id, position).unwrap();
                }
                app.update();
                render_y(&app, unit_id)
            };

            let ground_y = sample(ground, anchor_y + FLOOR_OFFSET_METERS);
            let upper_y = sample(upper, anchor_y + FLOOR_OFFSET_METERS + 3.0);
            assert!(
                ((upper_y - ground_y) - 3.0).abs() < 0.01,
                "regions in one building must differ by their authored metric height, \
                 got {ground_y} and {upper_y}"
            );
        }

        #[test]
        fn entering_and_leaving_an_interior_keeps_the_render_entity_and_children() {
            let mut app = scaled_app();
            let unit_id = prepare_resident_unit(&mut app, 1, 2);
            let (building_id, anchor_y) = place_hut(&mut app);
            let space_id = add_region(&mut app, building_id, anchor_y + FLOOR_OFFSET_METERS);
            app.update();

            let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
            let child = app.world_mut().spawn(Visibility::default()).id();
            app.world_mut().entity_mut(entity).add_child(child);
            let surface_y = render_y(&app, unit_id);

            {
                let mut world = app.world_mut().resource_mut::<WorldData>();
                world.set_unit_current_space(unit_id, space_id).unwrap();
                let mut position = world.get_unit(unit_id).unwrap().placement.position;
                position.local.0.y = anchor_y + FLOOR_OFFSET_METERS;
                world.relocate_unit(unit_id, position).unwrap();
            }
            app.update();

            assert_eq!(
                app.world().resource::<UnitRenderIndex>().0[&unit_id],
                entity,
                "entering an interior must not respawn the render entity"
            );
            assert!(app.world().get_entity(child).is_ok(), "child must survive");
            assert!(
                app.world()
                    .entity(entity)
                    .get::<Children>()
                    .is_some_and(|children| children.contains(&child)),
                "visual hierarchy must stay attached"
            );
            assert!(
                !matches!(
                    app.world().entity(entity).get::<Visibility>(),
                    Some(Visibility::Hidden)
                ),
                "no system may hide a unit for being indoors"
            );

            {
                let mut world = app.world_mut().resource_mut::<WorldData>();
                world
                    .set_unit_current_space(unit_id, SpaceId::SURFACE)
                    .unwrap();
                let mut position = world.get_unit(unit_id).unwrap().placement.position;
                position.local.0.y = 8.0;
                world.relocate_unit(unit_id, position).unwrap();
            }
            app.update();
            assert!(
                (render_y(&app, unit_id) - surface_y).abs() < 1e-3,
                "leaving must restore the plain terrain mapping"
            );
            assert_eq!(
                app.world().resource::<UnitRenderIndex>().0[&unit_id],
                entity
            );
        }
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
                WorldPosition::new(ChunkCoord::new(1, 1), LocalPosition::new(Vec3::ZERO)),
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
    fn death_pipeline_leaves_render_entity_after_world_removal() {
        let mut app = setup_sync_with_death_presentation_app();
        let unit_id = prepare_resident_unit(&mut app, 2, 2);
        app.update();
        let entity = *app
            .world()
            .resource::<UnitRenderIndex>()
            .0
            .get(&unit_id)
            .unwrap();

        let catalog = app.world().resource::<UnitCatalog>().clone();
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            world.damage_unit(unit_id, 999).unwrap();
            crate::world::step_unit_death_pipeline(
                &mut world,
                &catalog,
                None,
                &crate::world::CorpseSettings::default(),
                1,
            );
        }
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
        assert!(app.world().get_entity(entity).is_ok());
        assert!(app.world().entity(entity).contains::<DeathPresentation>());
    }

    #[test]
    fn removed_unit_record_enters_death_presentation() {
        let mut app = setup_sync_with_death_presentation_app();
        let unit_id = prepare_resident_unit(&mut app, 2, 2);
        app.update();
        assert_eq!(app.world().resource::<UnitRenderIndex>().0.len(), 1);
        let entity = *app
            .world()
            .resource::<UnitRenderIndex>()
            .0
            .get(&unit_id)
            .unwrap();

        app.world_mut()
            .resource_mut::<WorldData>()
            .remove_unit_by_id(unit_id);
        app.update();
        assert!(app.world().resource::<UnitRenderIndex>().0.is_empty());
        assert!(app.world().get_entity(entity).is_ok());
        assert!(app.world().entity(entity).contains::<DeathPresentation>());
    }
}
