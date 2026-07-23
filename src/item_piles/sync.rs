use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{
    ItemCatalog, ItemDefinition, ItemPileId, WorldData, pile_item_definition_id,
};

use super::assets::ItemSceneAssets;
use super::components::ItemPileRenderEntity;
use super::presentation::{
    ItemPileFallbackAssets, ItemPileFallbackMesh, ItemPileFallbackReason,
    ItemPilePresentationSettings, ItemPileSceneRoot,
};
use super::spawn::{
    despawn_item_pile_render_entities, pile_entity_name, spawn_item_pile_fallback_entity,
    spawn_item_pile_scene_entity,
};

/// Index of pile render entities.
#[derive(Resource, Default, Debug)]
pub struct ItemPileRenderIndex(pub std::collections::HashMap<ItemPileId, Entity>);

/// Systems that sync item pile render entities.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ItemPileRuntimeSystems;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemPileVisualKind {
    Scene,
    Fallback,
}

/// Collect pile ids that should have render entities this frame.
pub(crate) fn visible_item_pile_ids(
    world: &WorldData,
    residency: &ChunkResidencyTracker,
) -> HashSet<ItemPileId> {
    let mut visible = HashSet::new();
    for pile_id in world.item_pile_store().sorted_item_pile_ids() {
        let Some(chunk) = world.item_pile_store().pile_chunk(pile_id) else {
            continue;
        };
        if residency.is_resident(chunk) {
            visible.insert(pile_id);
        }
    }
    visible
}

/// Keep derived pile entities aligned with [`WorldData`] chunk residency (IA0).
pub fn sync_item_pile_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    config: Res<crate::world::WorldConfig>,
    items: Res<ItemCatalog>,
    residency: Res<ChunkResidencyTracker>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<ItemSceneAssets>,
    mut fallback_assets: ResMut<ItemPileFallbackAssets>,
    presentation: Res<ItemPilePresentationSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut index: ResMut<ItemPileRenderIndex>,
    existing: Query<(
        Entity,
        &ItemPileRenderEntity,
        Option<&ItemPileSceneRoot>,
        Option<&ItemPileFallbackMesh>,
    )>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let should_render = visible_item_pile_ids(&world, &residency);

    let stale: Vec<ItemPileId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    despawn_item_pile_render_entities(&mut commands, &mut index, stale);

    for (entity, marker, scene_root, fallback_mesh) in &existing {
        if !should_render.contains(&marker.pile_id) {
            continue;
        }
        let Some(record) = world.item_pile_store().get(marker.pile_id) else {
            commands.entity(entity).despawn();
            index.0.remove(&marker.pile_id);
            continue;
        };
        let translation = world_position_to_render_global(
            record.placement,
            config.chunk_layout(),
            vertical_scale,
        );
        let definition = resolve_pile_definition(&world, &items, record);
        let desired = resolve_desired_visual(definition, &mut scene_assets, &asset_server);
        let current = visual_kind(scene_root, fallback_mesh);
        if current != Some(desired.kind) {
            commands.entity(entity).despawn();
            index.0.remove(&marker.pile_id);
            continue;
        }
        let translation = if desired.kind == ItemPileVisualKind::Fallback {
            translation + Vec3::Y * presentation.fallback_sphere_radius
        } else {
            translation
        };
        commands.entity(entity).insert((
            Transform::from_translation(translation),
            Name::new(pile_entity_name(record, definition)),
        ));
    }

    for pile_id in should_render {
        if index.0.contains_key(&pile_id) {
            continue;
        }
        let Some(record) = world.item_pile_store().get(pile_id) else {
            continue;
        };
        let translation = world_position_to_render_global(
            record.placement,
            config.chunk_layout(),
            vertical_scale,
        );
        let definition = resolve_pile_definition(&world, &items, record);
        let desired = resolve_desired_visual(definition, &mut scene_assets, &asset_server);
        let label = pile_entity_name(record, definition);
        let entity = match desired.kind {
            ItemPileVisualKind::Scene => spawn_item_pile_scene_entity(
                &mut commands,
                pile_id,
                &label,
                desired.scene.expect("scene handle required"),
                translation,
            ),
            ItemPileVisualKind::Fallback => {
                let unique = record.stack_quantity().is_none();
                let mesh = fallback_assets.mesh(&mut meshes, &presentation);
                let material = fallback_assets.material_for_definition(
                    &mut materials,
                    &presentation,
                    definition,
                    unique,
                );
                spawn_item_pile_fallback_entity(
                    &mut commands,
                    pile_id,
                    &label,
                    translation,
                    mesh,
                    material,
                    desired.reason,
                    &presentation,
                )
            }
        };
        index.0.insert(pile_id, entity);
    }
}

struct DesiredVisual {
    kind: ItemPileVisualKind,
    scene: Option<Handle<Scene>>,
    reason: ItemPileFallbackReason,
}

fn resolve_desired_visual(
    definition: Option<&ItemDefinition>,
    scene_assets: &mut ItemSceneAssets,
    asset_server: &AssetServer,
) -> DesiredVisual {
    let Some(definition) = definition else {
        return DesiredVisual {
            kind: ItemPileVisualKind::Fallback,
            scene: None,
            reason: ItemPileFallbackReason::MissingDefinition,
        };
    };
    let Some(scene) = scene_assets.ensure_scene(
        &definition.id,
        &definition.render_key,
        asset_server,
    ) else {
        return DesiredVisual {
            kind: ItemPileVisualKind::Fallback,
            scene: None,
            reason: ItemPileFallbackReason::MissingRenderKey,
        };
    };
    if scene_is_loaded(asset_server, &scene) {
        DesiredVisual {
            kind: ItemPileVisualKind::Scene,
            scene: Some(scene),
            reason: ItemPileFallbackReason::SceneNotReady,
        }
    } else {
        DesiredVisual {
            kind: ItemPileVisualKind::Fallback,
            scene: None,
            reason: ItemPileFallbackReason::SceneNotReady,
        }
    }
}

fn resolve_pile_definition<'a>(
    world: &'a WorldData,
    items: &'a ItemCatalog,
    record: &crate::world::WorldItemPileRecord,
) -> Option<&'a ItemDefinition> {
    let definition_id = pile_item_definition_id(record, |instance_id| {
        world
            .item_instance_store()
            .get(instance_id)
            .map(|instance| instance.definition_id.clone())
    });
    definition_id
        .as_ref()
        .and_then(|id| items.get(id))
}

fn visual_kind(
    scene_root: Option<&ItemPileSceneRoot>,
    fallback_mesh: Option<&ItemPileFallbackMesh>,
) -> Option<ItemPileVisualKind> {
    if scene_root.is_some() {
        Some(ItemPileVisualKind::Scene)
    } else if fallback_mesh.is_some() {
        Some(ItemPileVisualKind::Fallback)
    } else {
        None
    }
}

fn scene_is_loaded(asset_server: &AssetServer, scene: &Handle<Scene>) -> bool {
    matches!(asset_server.get_load_state(scene), Some(LoadState::Loaded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::ChunkResidencyTracker;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, ItemDefinitionId,
        ItemPileSource, LocalPosition, SpaceId, WorldItemPileRecord, WorldPosition,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    #[test]
    fn visible_piles_require_resident_chunk() {
        let mut world = WorldData::new(layout());
        let samples = vec![0.0; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        world.insert(chunk, ChunkData::new(heightfield, Vec::new()));
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        world
            .item_pile_store_mut()
            .insert(
                chunk,
                WorldItemPileRecord::new_stack(
                    pile_id,
                    WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO)),
                    SpaceId::SURFACE,
                    ItemDefinitionId::new("gold"),
                    5,
                    None,
                    None,
                    crate::world::Affiliation::Player,
                    ItemPileSource::DevSpawned,
                    0,
                ),
            )
            .unwrap();
        let residency = ChunkResidencyTracker::default();
        assert!(visible_item_pile_ids(&world, &residency).is_empty());
        let mut residency = ChunkResidencyTracker::default();
        residency.mark_resident(chunk);
        assert_eq!(visible_item_pile_ids(&world, &residency).len(), 1);
    }
}
