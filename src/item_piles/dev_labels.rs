//! Dev-mode floating labels for world item piles (IA0).

use bevy::prelude::*;

use crate::camera::RtsCamera;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{ItemCatalog, ItemPileId, WorldConfig, WorldData, pile_item_definition_id};

use super::presentation::{ItemPilePresentationSettings, pile_display_metadata};
use super::sync::ItemPileRenderIndex;

#[derive(Component, Debug)]
pub struct ItemPileDevLabel {
    pub pile_id: ItemPileId,
}

#[derive(Resource, Default, Debug)]
pub struct ItemPileDevLabelIndex(pub std::collections::HashMap<ItemPileId, Entity>);

/// Spawn or update floating labels for visible piles when dev mode is active.
#[cfg(feature = "dev")]
pub fn sync_item_pile_dev_labels(
    mut commands: Commands,
    dev_state: Option<Res<crate::dev::DevModeState>>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    items: Res<ItemCatalog>,
    index: Res<ItemPileRenderIndex>,
    presentation: Res<ItemPilePresentationSettings>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut label_index: ResMut<ItemPileDevLabelIndex>,
    labels: Query<Entity, With<ItemPileDevLabel>>,
) {
    let Some(dev_state) = dev_state else {
        return;
    };

    if !dev_state.enabled {
        for entity in &labels {
            commands.entity(entity).despawn();
        }
        label_index.0.clear();
        return;
    }

    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    let mut desired = std::collections::HashSet::new();
    for pile_id in index.0.keys() {
        desired.insert(*pile_id);
    }

    let stale: Vec<ItemPileId> = label_index
        .0
        .keys()
        .copied()
        .filter(|id| !desired.contains(id))
        .collect();
    for pile_id in stale {
        if let Some(entity) = label_index.0.remove(&pile_id) {
            commands.entity(entity).despawn();
        }
    }

    for pile_id in desired {
        let Some(record) = world.item_pile_store().get(pile_id) else {
            continue;
        };
        let definition_id = pile_item_definition_id(record, |instance_id| {
            world
                .item_instance_store()
                .get(instance_id)
                .map(|instance| instance.definition_id.clone())
        });
        let (label, _) = pile_display_metadata(
            definition_id.as_ref(),
            &items,
            record.stack_quantity(),
        );
        let translation = world_position_to_render_global(
            record.placement,
            config.chunk_layout(),
            vertical_scale,
        ) + Vec3::Y * presentation.dev_label_offset_y;

        if let Some(entity) = label_index.0.get(&pile_id).copied() {
            commands.entity(entity).insert((
                Transform::from_translation(translation),
                Text2d::new(label),
            ));
            continue;
        }

        let entity = commands
            .spawn((
                ItemPileDevLabel { pile_id },
                Text2d::new(label),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgba(0.95, 0.98, 1.0, 0.95)),
                Transform::from_translation(translation),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .id();
        label_index.0.insert(pile_id, entity);
    }
}

/// Billboard dev labels toward the active RTS camera.
#[cfg(feature = "dev")]
pub fn billboard_item_pile_dev_labels(
    camera: Query<&GlobalTransform, With<RtsCamera>>,
    mut labels: Query<&mut Transform, With<ItemPileDevLabel>>,
) {
    let Ok(camera_transform) = camera.single() else {
        return;
    };
    let camera_position = camera_transform.translation();
    for mut transform in &mut labels {
        let label_world = transform.translation;
        let to_camera = camera_position - label_world;
        if to_camera.length_squared() < 1e-6 {
            continue;
        }
        let forward = to_camera.normalize();
        let mut right = Vec3::Y.cross(forward);
        if right.length_squared() < 1e-6 {
            right = Vec3::X;
        } else {
            right = right.normalize();
        }
        let up = forward.cross(right);
        transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
    }
}

#[cfg(not(feature = "dev"))]
pub fn sync_item_pile_dev_labels() {}

#[cfg(not(feature = "dev"))]
pub fn billboard_item_pile_dev_labels() {}
