//! Spawn and despawn derived item pile render entities (IA0).

use bevy::prelude::*;

use crate::world::{ItemDefinition, ItemPileId, WorldItemPileRecord};

use super::components::ItemPileRenderEntity;
use super::presentation::{
    ItemPileFallbackMesh, ItemPileFallbackReason, ItemPilePresentationSettings, ItemPileSceneRoot,
};

/// Spawn an authored glTF scene for a world pile.
pub fn spawn_item_pile_scene_entity(
    commands: &mut Commands,
    pile_id: ItemPileId,
    label: &str,
    scene: Handle<Scene>,
    translation: Vec3,
) -> Entity {
    commands
        .spawn((
            ItemPileRenderEntity { pile_id },
            ItemPileSceneRoot,
            Name::new(label.to_string()),
            SceneRoot(scene),
            Transform::from_translation(translation),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id()
}

/// Spawn a generic fallback sphere for a world pile.
pub fn spawn_item_pile_fallback_entity(
    commands: &mut Commands,
    pile_id: ItemPileId,
    label: &str,
    translation: Vec3,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    reason: ItemPileFallbackReason,
    settings: &ItemPilePresentationSettings,
) -> Entity {
    let y_offset = settings.fallback_sphere_radius;
    commands
        .spawn((
            ItemPileRenderEntity { pile_id },
            ItemPileFallbackMesh { reason },
            Name::new(label.to_string()),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(translation + Vec3::Y * y_offset),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id()
}

/// Despawn render entities tracked in `index` for the given pile ids.
pub fn despawn_item_pile_render_entities(
    commands: &mut Commands,
    index: &mut super::sync::ItemPileRenderIndex,
    ids: impl IntoIterator<Item = ItemPileId>,
) {
    for id in ids {
        if let Some(entity) = index.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Build a debug-friendly entity name from pile metadata.
pub fn pile_entity_name(
    record: &WorldItemPileRecord,
    definition: Option<&ItemDefinition>,
) -> String {
    let display_name = definition
        .map(|def| def.display_name.as_str())
        .unwrap_or("Unknown Item");
    match record.stack_quantity() {
        Some(quantity) => format!("{display_name} x{quantity}"),
        None => display_name.to_string(),
    }
}
