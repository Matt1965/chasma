//! Spawn and despawn derived doodad render entities (ADR-023).

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global;
use crate::world::{ChunkId, DoodadId, DoodadRecord, WorldConfig};

use super::components::DoodadRenderEntity;

/// Spawn a glTF scene entity for an authoritative doodad record.
pub fn spawn_doodad_render_entity(
    commands: &mut Commands,
    record: &DoodadRecord,
    chunk_id: ChunkId,
    scene: Handle<Scene>,
    config: &WorldConfig,
    vertical_scale: f32,
) -> Entity {
    let layout = config.chunk_layout();
    let translation =
        world_position_to_render_global(record.placement.position, layout, vertical_scale);
    commands
        .spawn((
            DoodadRenderEntity {
                doodad_id: record.id,
                chunk_id,
            },
            SceneRoot(scene),
            Transform {
                translation,
                rotation: record.placement.rotation,
                scale: record.placement.scale,
            },
            Visibility::default(),
        ))
        .id()
}

/// Despawn all render entities tracked in `index` for the given doodad ids.
pub fn despawn_doodad_render_entities(
    commands: &mut Commands,
    index: &mut DoodadRenderIndex,
    ids: impl IntoIterator<Item = DoodadId>,
) {
    for id in ids {
        if let Some(entity) = index.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Maps authoritative doodad ids to derived render entities.
#[derive(Debug, Resource, Default)]
pub struct DoodadRenderIndex(pub std::collections::HashMap<DoodadId, Entity>);
