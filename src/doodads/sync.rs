//! Sync doodad render entities with authoritative world data and terrain residency (ADR-023).

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{world_position_to_render_global, TerrainRenderAssets};
use crate::world::{DoodadCatalog, DoodadId, WorldConfig, WorldData};

use super::assets::DoodadSceneAssets;
use super::components::DoodadRenderEntity;
use super::spawn::{DoodadRenderIndex, despawn_doodad_render_entities, spawn_doodad_render_entity};

/// Systems that sync doodad render entities with world data.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DoodadRuntimeSystems;

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
) {
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let mut should_render: HashSet<DoodadId> = HashSet::new();

    for (chunk_id, _) in world.iter() {
        if !residency.is_resident(chunk_id) {
            continue;
        }
        let Some(store) = world.doodads_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            should_render.insert(record.id);
        }
    }

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
        commands.entity(entity).insert(Transform {
            translation,
            rotation: record.placement.rotation,
            scale: record.placement.scale,
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
        let Some(scene) = scene_assets.scene_for(&definition.id).cloned() else {
            if let Some(key) = definition.render_key.0.as_deref() {
                scene_assets.log_missing_once(key);
            }
            continue;
        };
        if !scene_is_loaded(&asset_server, &scene) {
            continue;
        }

        let entity = spawn_doodad_render_entity(
            &mut commands,
            record,
            chunk_id,
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
