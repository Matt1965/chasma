//! Spawn and despawn derived unit render entities (ADR-028).

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global_above_base;
use crate::world::unit_visual_rotation;
use crate::world::{
    UnitDefinition, UnitId, UnitRecord, WorldConfig, WorldData, space_vertical_reference_y,
};

use super::components::{UnitRenderEntity, UnitRenderMetadata, UnitSceneRoot, UnitVisualFacing};

/// Render translation for a unit, honoring the space its authoritative Y belongs to.
///
/// A unit standing on an interior floor is positioned above the building anchor by an
/// authored metric offset. Exaggerating that offset with the terrain vertical scale
/// throws the unit far above or below the visible floor (IN-11c).
pub fn unit_render_translation(
    world: &WorldData,
    record: &UnitRecord,
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

/// Spawn a glTF scene entity for an authoritative unit record.
///
/// `visual_scale` is the composed presentation scale (definition baseline × instance;
/// units have no instance scale today — pass [`crate::world::unit_visual_scale`]).
pub fn spawn_unit_render_entity(
    commands: &mut Commands,
    world: &WorldData,
    record: &UnitRecord,
    definition: &UnitDefinition,
    scene: Handle<Scene>,
    config: &WorldConfig,
    vertical_scale: f32,
    visual_scale: Vec3,
) -> Entity {
    let layout = config.chunk_layout();
    let translation = unit_render_translation(world, record, layout, vertical_scale);
    commands
        .spawn((
            UnitRenderEntity { unit_id: record.id },
            UnitRenderMetadata {
                definition_id: record.definition_id.clone(),
            },
            UnitVisualFacing {
                rotation: record.placement.rotation,
            },
            UnitSceneRoot,
            SceneRoot(scene),
            Transform {
                translation,
                rotation: unit_visual_rotation(definition, record.placement.rotation),
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
