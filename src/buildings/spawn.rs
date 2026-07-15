//! Spawn and despawn derived building render entities (ADR-079 B2, ADR-095 BA1).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global;
use crate::world::{
    BuildingDefinition, BuildingRecord, ChunkId, WorldConfig, building_anchor_render_transform,
    building_has_model_correction, building_model_correction_local_transform,
    building_model_render_transform,
};

use super::components::{BuildingRenderEntity, BuildingSceneRoot};
use super::fallback::{
    BuildingFallbackAssets, BuildingFallbackReason, spawn_diagnostic_fallback_entity,
};
use super::placeholder::placeholder_mesh_size;

/// Maps authoritative building ids to derived render entities.
#[derive(Debug, Resource, Default)]
pub struct BuildingRenderIndex(pub HashMap<crate::world::BuildingId, Entity>);

/// Authoritative placement → render translation (ground anchor, no cuboid half-height bump).
pub fn building_render_translation(
    record: &BuildingRecord,
    config: &WorldConfig,
    vertical_scale: f32,
) -> Vec3 {
    world_position_to_render_global(
        record.placement.position,
        config.chunk_layout(),
        vertical_scale,
    )
}

/// Diagnostic fallback uses cuboid center offset so the footprint base sits on the anchor.
pub fn diagnostic_fallback_translation(
    record: &BuildingRecord,
    definition: &BuildingDefinition,
    config: &WorldConfig,
    vertical_scale: f32,
) -> Vec3 {
    let mut translation = building_render_translation(record, config, vertical_scale);
    let mesh_size = placeholder_mesh_size(definition);
    translation.y += mesh_size.y * 0.5;
    translation
}

/// Spawn a glTF scene root for an authoritative building record.
pub fn spawn_building_scene_entity(
    commands: &mut Commands,
    record: &BuildingRecord,
    definition: &BuildingDefinition,
    chunk_id: ChunkId,
    scene: Handle<Scene>,
    render_key: String,
    config: &WorldConfig,
    vertical_scale: f32,
) -> Entity {
    let layout = config.chunk_layout();
    let marker = (
        BuildingRenderEntity {
            building_id: record.id,
            chunk_id,
            lifecycle_state: record.lifecycle_state,
            active_render_key: Some(render_key),
            uses_diagnostic_fallback: false,
        },
        BuildingSceneRoot,
        Visibility::default(),
    );

    if building_has_model_correction(definition) {
        let anchor = building_anchor_render_transform(
            definition,
            &record.placement,
            layout,
            vertical_scale,
        );
        let correction = building_model_correction_local_transform(definition);
        return commands
            .spawn((marker, anchor))
            .with_children(|parent| {
                parent.spawn((SceneRoot(scene), correction));
            })
            .id();
    }

    let transform = building_model_render_transform(
        definition,
        &record.placement,
        layout,
        vertical_scale,
    );
    commands
        .spawn((marker, SceneRoot(scene), transform))
        .id()
}

/// Spawn a diagnostic fallback cuboid when no valid GLB is available.
pub fn spawn_building_fallback_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fallback_assets: &mut BuildingFallbackAssets,
    record: &BuildingRecord,
    definition: &BuildingDefinition,
    chunk_id: ChunkId,
    config: &WorldConfig,
    vertical_scale: f32,
    reason: BuildingFallbackReason,
) -> Entity {
    let marker = BuildingRenderEntity {
        building_id: record.id,
        chunk_id,
        lifecycle_state: record.lifecycle_state,
        active_render_key: None,
        uses_diagnostic_fallback: true,
    };
    let translation = diagnostic_fallback_translation(record, definition, config, vertical_scale);
    spawn_diagnostic_fallback_entity(
        commands,
        meshes,
        materials,
        fallback_assets,
        record,
        definition,
        marker,
        translation,
        reason,
    )
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

#[cfg(test)]
pub fn spawn_building_render_entity_for_test(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    scene_assets: &mut super::assets::BuildingSceneAssets,
    _fallback_assets: &mut BuildingFallbackAssets,
    asset_server: &AssetServer,
    record: &BuildingRecord,
    catalog: &crate::world::BuildingCatalog,
    chunk_id: ChunkId,
    config: &WorldConfig,
) -> Entity {
    use super::assets::lifecycle_render_key;

    let definition = catalog
        .get(&record.definition_id)
        .expect("definition exists");
    let render_key = lifecycle_render_key(definition, record.lifecycle_state).unwrap();
    let scene = scene_assets
        .ensure_scene(&render_key, asset_server)
        .expect("scene handle");
    spawn_building_scene_entity(
        commands, record, definition, chunk_id, scene, render_key, config, 1.0,
    )
}
