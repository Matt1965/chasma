use std::collections::HashMap;

use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{ItemCatalog, ItemPileId, WorldConfig, WorldData};

use super::components::ItemPileRenderEntity;

/// Index of pile render entities.
#[derive(Resource, Default, Debug)]
pub struct ItemPileRenderIndex(pub HashMap<ItemPileId, Entity>);

/// Systems that sync item pile render entities.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ItemPileRuntimeSystems;

/// Keep derived pile entities aligned with [`WorldData`] chunk residency.
pub fn sync_item_pile_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    items: Res<ItemCatalog>,
    residency: Res<ChunkResidencyTracker>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut index: ResMut<ItemPileRenderIndex>,
    existing: Query<(Entity, &ItemPileRenderEntity)>,
) {
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let mut should_render = std::collections::HashSet::new();
    for (chunk_id, _) in world.iter() {
        if !residency.is_resident(chunk_id) {
            continue;
        }
        for pile in world.item_pile_store().piles_in_chunk(chunk_id) {
            should_render.insert(pile.id);
        }
    }

    let stale: Vec<ItemPileId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    for pile_id in stale {
        if let Some(entity) = index.0.remove(&pile_id) {
            commands.entity(entity).despawn();
        }
    }

    for pile_id in should_render {
        if index.0.contains_key(&pile_id) {
            continue;
        }
        let Some(record) = world.item_pile_store().get(pile_id) else {
            continue;
        };
        let definition_id = crate::world::pile_item_definition_id(record, |instance_id| {
            world
                .item_instance_store()
                .get(instance_id)
                .map(|instance| instance.definition_id.clone())
        });
        let label = definition_id
            .as_ref()
            .and_then(|id| items.get(id))
            .map(|def| def.display_name.clone())
            .unwrap_or_else(|| "Item Pile".to_string());
        let quantity = record.stack_quantity().unwrap_or(1);
        let translation = world_position_to_render_global(
            record.placement,
            config.chunk_layout(),
            vertical_scale,
        );
        let entity = commands
            .spawn((
                ItemPileRenderEntity { pile_id },
                Name::new(format!("{label} x{quantity}")),
                Transform::from_translation(translation),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .id();
        index.0.insert(pile_id, entity);
    }

    for (entity, marker) in &existing {
        if !index.0.contains_key(&marker.pile_id) {
            index.0.insert(marker.pile_id, entity);
        }
    }
}
