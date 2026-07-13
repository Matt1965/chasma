//! Sync building render entities with authoritative world data and terrain residency (ADR-079 B2).

use std::collections::HashSet;

use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{BuildingCatalog, BuildingId, WorldConfig, WorldData};

use super::components::BuildingRenderEntity;
use super::placeholder::{lifecycle_building_color, placeholder_mesh_size};
use super::spawn::{
    BuildingRenderAssets, BuildingRenderIndex, despawn_building_render_entities,
    spawn_building_render_entity,
};

/// Systems that sync building render entities with world data.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct BuildingRuntimeSystems;

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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut render_assets: ResMut<BuildingRenderAssets>,
    mut index: ResMut<BuildingRenderIndex>,
    existing: Query<(
        Entity,
        &BuildingRenderEntity,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    render_terrain: Option<Res<TerrainRenderAssets>>,
) {
    let vertical_scale = render_terrain
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let should_render = visible_building_ids(&world, &residency);

    let stale: Vec<BuildingId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    despawn_building_render_entities(&mut commands, &mut index, stale);

    for (entity, marker, material_handle) in &existing {
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
        let layout = config.chunk_layout();
        let mut translation =
            world_position_to_render_global(record.placement.position, layout, vertical_scale);
        let mesh_size = placeholder_mesh_size(definition);
        translation.y += mesh_size.y * 0.5;
        commands.entity(entity).insert(Transform {
            translation,
            rotation: record.placement.rotation,
            scale: Vec3::ONE,
        });
        if marker.lifecycle_state != record.lifecycle_state {
            let color =
                lifecycle_building_color(record.lifecycle_state, record.ownership.affiliation);
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.base_color = color;
            }
            commands.entity(entity).insert(BuildingRenderEntity {
                building_id: marker.building_id,
                chunk_id: marker.chunk_id,
                lifecycle_state: record.lifecycle_state,
            });
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
            continue;
        };

        let entity = spawn_building_render_entity(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut render_assets,
            record,
            definition,
            chunk_id,
            &config,
            vertical_scale,
        );
        index.0.insert(id, entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildings::spawn::spawn_building_render_entity_for_test;
    use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
    use crate::world::{
        BuildingCatalog, BuildingDefinitionId, BuildingId, BuildingOwnership, BuildingPlacement,
        BuildingRecord, BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield,
        LocalPosition, WorldConfig, WorldData, WorldPosition,
    };
    use bevy::prelude::{App, MinimalPlugins, Quat, StandardMaterial, Update, Vec3};

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
        app.add_plugins(MinimalPlugins);
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<BuildingCatalog>();
        app.init_resource::<ChunkResidencyTracker>();
        app.init_resource::<BuildingRenderIndex>();
        app.init_resource::<BuildingRenderAssets>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.add_systems(Update, sync_building_render_entities);
        app
    }

    fn prepare_resident_building(app: &mut App, x: i32, z: i32) -> BuildingId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, x, z);
            insert_authored_building(&mut world, x, z)
        };
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
    fn sync_spawns_render_entity_for_resident_record() {
        let mut app = setup_sync_app();
        let building_id = prepare_resident_building(&mut app, 1, 2);
        app.update();

        let index = app.world().resource::<BuildingRenderIndex>();
        assert_eq!(index.0.len(), 1);
        assert!(index.0.contains_key(&building_id));
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
    fn sync_transform_matches_world_data_with_vertical_scale() {
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
        let catalog = app.world().resource::<BuildingCatalog>();
        let definition = catalog.get(&record.definition_id).unwrap();
        let mut expected = world_position_to_render_global(
            record.placement.position,
            config.chunk_layout(),
            vertical_scale,
        );
        expected.y += super::super::placeholder::placeholder_mesh_size(definition).y * 0.5;
        assert_eq!(transform.translation, expected);
        assert_eq!(record.placement.position.local.0.y, 12.0);
    }

    #[test]
    fn world_data_remains_authoritative_after_sync() {
        let mut app = setup_sync_app();
        let building_id = prepare_resident_building(&mut app, 9, 10);
        app.update();

        let hp = app
            .world()
            .resource::<WorldData>()
            .get_building(building_id)
            .unwrap()
            .vitals
            .current_hp;
        assert_eq!(hp, 250);
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
