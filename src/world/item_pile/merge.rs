use bevy::prelude::*;

use super::id::ItemPileId;
use super::record::{WorldItemPileRecord, WorldPileContents};
use super::settings::ItemPileSettings;
use crate::world::ownership::{Affiliation, OwnerId, TeamId};
use crate::world::{ItemDefinitionId, SpaceId, WorldPosition};

/// Deterministic overflow offsets around a drop point (meters, local XZ).
pub const OVERFLOW_PILE_OFFSETS: [(f32, f32); 8] = [
    (0.0, 0.0),
    (0.45, 0.0),
    (-0.45, 0.0),
    (0.0, 0.45),
    (0.0, -0.45),
    (0.35, 0.35),
    (-0.35, 0.35),
    (0.35, -0.35),
];

pub fn quantized_distance_squared_cm(a: WorldPosition, b: WorldPosition) -> i64 {
    let dx = ((a.local.0.x - b.local.0.x) * 100.0).round() as i64;
    let dz = ((a.local.0.z - b.local.0.z) * 100.0).round() as i64;
    dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz))
}

pub fn piles_can_merge(
    a: &WorldItemPileRecord,
    b: &WorldItemPileRecord,
    item_definition_id: &ItemDefinitionId,
) -> bool {
    if a.current_space_id != b.current_space_id {
        return false;
    }
    if !ownership_compatible(a, b) {
        return false;
    }
    match (&a.contents, &b.contents) {
        (
            WorldPileContents::Stack {
                item_definition_id: id_a,
                ..
            },
            WorldPileContents::Stack {
                item_definition_id: id_b,
                ..
            },
        ) => id_a == id_b && id_a == item_definition_id,
        _ => false,
    }
}

pub fn ownership_compatible(a: &WorldItemPileRecord, b: &WorldItemPileRecord) -> bool {
    a.owner_id == b.owner_id && a.team_id == b.team_id && a.affiliation == b.affiliation
}

pub fn merge_candidate_order(
    drop_position: WorldPosition,
    space_id: SpaceId,
    item_definition_id: &ItemDefinitionId,
    piles: &[WorldItemPileRecord],
    settings: &ItemPileSettings,
) -> Vec<ItemPileId> {
    let max_dist_sq = settings.merge_radius_squared_cm();
    let mut candidates: Vec<(i64, ItemPileId)> = piles
        .iter()
        .filter(|pile| {
            pile.current_space_id == space_id
                && matches!(
                    &pile.contents,
                    WorldPileContents::Stack {
                        item_definition_id: id,
                        ..
                    } if id == item_definition_id
                )
        })
        .filter_map(|pile| {
            let dist_sq = quantized_distance_squared_cm(drop_position, pile.placement);
            (dist_sq <= max_dist_sq).then_some((dist_sq, pile.id))
        })
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.into_iter().map(|(_, id)| id).collect()
}

pub fn offset_position(base: WorldPosition, offset_index: usize) -> WorldPosition {
    let (ox, oz) = OVERFLOW_PILE_OFFSETS
        .get(offset_index % OVERFLOW_PILE_OFFSETS.len())
        .copied()
        .unwrap_or((0.0, 0.0));
    let mut pos = base;
    pos.local.0.x += ox;
    pos.local.0.z += oz;
    pos
}

pub fn unit_may_access_pile(
    pile: &WorldItemPileRecord,
    actor_owner: Option<OwnerId>,
    actor_team: Option<TeamId>,
    actor_affiliation: Affiliation,
) -> bool {
    match pile.affiliation {
        Affiliation::Neutral | Affiliation::Wildlife | Affiliation::Unknown => true,
        Affiliation::Player | Affiliation::Dev => {
            actor_affiliation == Affiliation::Player
                && pile
                    .owner_id
                    .map_or(true, |owner| actor_owner == Some(owner))
        }
        Affiliation::Hostile => actor_team == pile.team_id || actor_owner == pile.owner_id,
    }
}
