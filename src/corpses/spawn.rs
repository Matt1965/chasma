//! Spawn derived corpse render entities.

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global_above_base;
use crate::units::{UnitRenderMetadata, UnitVisualFacing};
use crate::world::unit_visual_rotation;
use crate::world::{
    CorpseId, CorpseRecord, UnitDefinition, WorldConfig, WorldData,
    space_vertical_reference_y,
};

use super::components::{CorpseDeathPosePending, CorpseRenderEntity, CorpseSceneRoot};

/// Render translation for a corpse record.
pub fn corpse_render_translation(
    world: &WorldData,
    record: &CorpseRecord,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Vec3 {
    world_position_to_render_global_above_base(
        record.placement.position,
        layout,
        vertical_scale,
        space_vertical_reference_y(world, world.space_registry(), record.current_space_id),
    )
}

/// Spawn a glTF scene entity for an authoritative corpse record.
pub fn spawn_corpse_render_entity(
    commands: &mut Commands,
    world: &WorldData,
    record: &CorpseRecord,
    definition: &UnitDefinition,
    scene: Handle<Scene>,
    config: &WorldConfig,
    vertical_scale: f32,
    visual_scale: Vec3,
    death_pose_pending: bool,
) -> Entity {
    let layout = config.chunk_layout();
    let translation = world_position_to_render_global_above_base(
        record.placement.position,
        layout,
        vertical_scale,
        space_vertical_reference_y(world, world.space_registry(), record.current_space_id),
    );
    let mut entity = commands.spawn((
        CorpseRenderEntity {
            corpse_id: record.id,
        },
        UnitRenderMetadata {
            definition_id: record.unit_definition_id.clone(),
        },
        UnitVisualFacing {
            rotation: record.placement.rotation,
        },
        CorpseSceneRoot,
        SceneRoot(scene),
        Transform {
            translation,
            rotation: unit_visual_rotation(definition, record.placement.rotation),
            scale: visual_scale,
        },
        Visibility::default(),
    ));
    if death_pose_pending {
        if let Some(profile_id) = definition.animation_profile_id.clone() {
            let freeze_pose = profile_id.as_str().is_empty();
            entity.insert(CorpseDeathPosePending {
                definition_id: record.unit_definition_id.clone(),
                profile_id,
                freeze_pose,
            });
        }
    }
    entity.id()
}

/// Despawn corpse render entities tracked in `index`.
pub fn despawn_corpse_render_entities(
    commands: &mut Commands,
    index: &mut super::sync::CorpseRenderIndex,
    ids: impl IntoIterator<Item = CorpseId>,
) {
    for id in ids {
        if let Some(entity) = index.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
}
