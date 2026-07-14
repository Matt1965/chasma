use super::id::ItemPileId;
use super::record::{WorldItemPileRecord, WorldPileContents};
use super::settings::ItemPileSettings;
use crate::world::{ItemDefinitionId, SpaceId, WorldPosition};

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

/// Piles in a chunk within merge radius of a position, sorted deterministically.
pub fn item_piles_near<'a>(
    piles: &'a [WorldItemPileRecord],
    position: WorldPosition,
    space_id: SpaceId,
    settings: &ItemPileSettings,
) -> Vec<&'a WorldItemPileRecord> {
    let max_dist_sq = settings.merge_radius_squared_cm();
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
