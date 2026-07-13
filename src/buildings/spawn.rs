//! Spawn and despawn derived building render entities (ADR-079 B2).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global;
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinition, BuildingLifecycleState, BuildingRecord,
    ChunkId, WorldConfig,
};

use super::components::BuildingRenderEntity;
use super::placeholder::{affiliation_color, lifecycle_building_color, placeholder_mesh_size};

/// Cached placeholder meshes and affiliation materials for stable recreation.
#[derive(Resource, Default)]
pub struct BuildingRenderAssets {
    meshes: HashMap<[u32; 3], Handle<Mesh>>,
    materials: HashMap<Affiliation, Handle<StandardMaterial>>,
}

impl BuildingRenderAssets {
    pub fn mesh_for_definition(
        &mut self,
        meshes: &mut Assets<Mesh>,
        definition: &BuildingDefinition,
    ) -> Handle<Mesh> {
        let size = placeholder_mesh_size(definition);
        let key = [
            (size.x * 100.0).round() as u32,
            (size.y * 100.0).round() as u32,
            (size.z * 100.0).round() as u32,
        ];
        self.meshes
            .entry(key)
            .or_insert_with(|| meshes.add(Cuboid::new(size.x, size.y, size.z)))
            .clone()
    }

    pub fn material_for_affiliation(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        affiliation: Affiliation,
    ) -> Handle<StandardMaterial> {
        self.materials
            .entry(affiliation)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: affiliation_color(affiliation),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })
            })
            .clone()
    }
}

/// Spawn a placeholder cuboid entity for an authoritative building record.
pub fn spawn_building_render_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    render_assets: &mut BuildingRenderAssets,
    record: &BuildingRecord,
    definition: &BuildingDefinition,
    chunk_id: ChunkId,
    config: &WorldConfig,
    vertical_scale: f32,
) -> Entity {
    let layout = config.chunk_layout();
    let mut translation =
        world_position_to_render_global(record.placement.position, layout, vertical_scale);
    let mesh_size = placeholder_mesh_size(definition);
    translation.y += mesh_size.y * 0.5;

    let mesh = render_assets.mesh_for_definition(meshes, definition);
    let material = materials.add(StandardMaterial {
        base_color: lifecycle_building_color(record.lifecycle_state, record.ownership.affiliation),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands
        .spawn((
            BuildingRenderEntity {
                building_id: record.id,
                chunk_id,
                lifecycle_state: record.lifecycle_state,
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform {
                translation,
                rotation: record.placement.rotation,
                scale: Vec3::ONE,
            },
            Visibility::default(),
        ))
        .id()
}

/// Despawn all render entities tracked in `index` for the given building ids.
pub fn despawn_building_render_entities(
    commands: &mut Commands,
    index: &mut BuildingRenderIndex,
    ids: impl IntoIterator<Item = crate::world::BuildingId>,
) {
    for id in ids {
        if let Some(entity) = index.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Maps authoritative building ids to derived render entities.
#[derive(Debug, Resource, Default)]
pub struct BuildingRenderIndex(pub HashMap<crate::world::BuildingId, Entity>);

#[cfg(test)]
pub fn spawn_building_render_entity_for_test(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    render_assets: &mut BuildingRenderAssets,
    record: &BuildingRecord,
    catalog: &BuildingCatalog,
    chunk_id: ChunkId,
    config: &WorldConfig,
) -> Entity {
    let definition = catalog
        .get(&record.definition_id)
        .expect("definition exists");
    spawn_building_render_entity(
        commands,
        meshes,
        materials,
        render_assets,
        record,
        definition,
        chunk_id,
        config,
        1.0,
    )
}
