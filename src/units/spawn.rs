//! Spawn and despawn derived unit render entities (ADR-028).

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global;
use crate::world::{UnitId, UnitRecord, WorldConfig};

use super::components::{UnitRenderEntity, UnitRenderMetadata, UnitSceneRoot};

/// Spawn a glTF scene entity for an authoritative unit record.
///
/// `visual_scale` is the composed presentation scale (definition baseline × instance;
/// units have no instance scale today — pass [`crate::world::unit_visual_scale`]).
pub fn spawn_unit_render_entity(
    commands: &mut Commands,
    record: &UnitRecord,
    scene: Handle<Scene>,
    config: &WorldConfig,
    vertical_scale: f32,
    visual_scale: Vec3,
) -> Entity {
    let layout = config.chunk_layout();
    let translation =
        world_position_to_render_global(record.placement.position, layout, vertical_scale);
    commands
        .spawn((
            UnitRenderEntity { unit_id: record.id },
            UnitRenderMetadata {
                definition_id: record.definition_id.clone(),
            },
            UnitSceneRoot,
            SceneRoot(scene),
            Transform {
                translation,
                rotation: record.placement.rotation,
                scale: visual_scale,
            },
            Visibility::default(),
        ))
        .id()
}

/// Despawn all render entities tracked in `index` for the given unit ids.
pub fn despawn_unit_render_entities(
    commands: &mut Commands,
    index: &mut UnitRenderIndex,
    ids: impl IntoIterator<Item = UnitId>,
) {
    for id in ids {
        if let Some(entity) = index.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Maps authoritative unit ids to derived render entities.
#[derive(Debug, Resource, Default)]
pub struct UnitRenderIndex(pub std::collections::HashMap<UnitId, Entity>);
