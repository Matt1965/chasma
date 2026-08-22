use super::id::ItemPileId;
use super::record::{WorldItemPileRecord, WorldPileContents};
use super::settings::ItemPileSettings;
use crate::world::{ChunkCoord, ChunkId, ItemDefinitionId, SpaceId, WorldData, WorldPosition};

/// Resolve the item definition for a pile's contents.
pub fn pile_item_definition_id(
    pile: &WorldItemPileRecord,
    instance_definition: impl Fn(crate::world::inventory::ItemInstanceId) -> Option<ItemDefinitionId>,
) -> Option<ItemDefinitionId> {
    match &pile.contents {
        WorldPileContents::Stack {
            item_definition_id, ..
        } => Some(item_definition_id.clone()),
        WorldPileContents::Unique { item_instance_id } => instance_definition(*item_instance_id),
    }
}

/// Piles within a horizontal radius of a position, sorted deterministically.
pub fn item_piles_within_radius<'a>(
    piles: &'a [WorldItemPileRecord],
    position: WorldPosition,
    space_id: SpaceId,
    max_dist_sq: i64,
) -> Vec<&'a WorldItemPileRecord> {
    let mut nearby: Vec<(i64, ItemPileId, &'a WorldItemPileRecord)> = piles
        .iter()
        .filter(|pile| pile.current_space_id == space_id)
        .filter_map(|pile| {
            let dist = super::merge::quantized_distance_squared_cm(position, pile.placement);
            (dist <= max_dist_sq).then_some((dist, pile.id, pile))
        })
        .collect();
    nearby.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    nearby.into_iter().map(|(_, _, pile)| pile).collect()
}

/// Piles in a chunk within merge radius of a position, sorted deterministically.
pub fn item_piles_near<'a>(
    piles: &'a [WorldItemPileRecord],
    position: WorldPosition,
    space_id: SpaceId,
    settings: &ItemPileSettings,
) -> Vec<&'a WorldItemPileRecord> {
    item_piles_within_radius(
        piles,
        position,
        space_id,
        settings.merge_radius_squared_cm(),
    )
}

/// Nearest pile at a world position within interaction radius (3×3 chunk scan).
pub fn nearest_item_pile_at_position<'a>(
    world: &'a WorldData,
    position: WorldPosition,
    space_id: SpaceId,
    settings: &ItemPileSettings,
) -> Option<&'a WorldItemPileRecord> {
    let max_dist_sq = settings.interaction_radius_squared_cm();
    let mut best: Option<(i64, ItemPileId, &'a WorldItemPileRecord)> = None;

    let mut chunks: Vec<ChunkCoord> = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            chunks.push(ChunkCoord::new(
                position.chunk.x + dx,
                position.chunk.z + dz,
            ));
        }
    }
    chunks.sort_by_key(|coord| (coord.x, coord.z));

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        for pile in world.item_pile_store().piles_in_chunk(chunk_id) {
            if pile.current_space_id != space_id {
                continue;
            }
            let dist = super::merge::quantized_distance_squared_cm(position, pile.placement);
            if dist > max_dist_sq {
                continue;
            }
            let replace = match &best {
                None => true,
                Some((best_dist, best_id, _)) => {
                    dist < *best_dist - 1 || (dist - *best_dist).abs() <= 1 && pile.id < *best_id
                }
            };
            if replace {
                best = Some((dist, pile.id, pile));
            }
        }
    }

    best.map(|(_, _, pile)| pile)
}
