//! Corpse presentation ownership — claims, stale release, spawn gating.

use bevy::prelude::*;

use crate::units::{DeathPresentation, UnitRenderEntity};
use crate::world::{CorpseId, CorpseState, UnitId, WorldData};

use super::components::{CorpsePresentationClaim, CorpseRenderEntity};
use super::sync::CorpseRenderIndex;

/// Claim an existing render root for a corpse while death presentation is active.
pub fn claim_corpse_presentation_for_death(
    corpse_index: &mut CorpseRenderIndex,
    commands: &mut Commands,
    entity: Entity,
    corpse_id: CorpseId,
    origin_unit_id: UnitId,
) {
    corpse_index.0.insert(corpse_id, entity);
    commands.entity(entity).insert(CorpsePresentationClaim {
        corpse_id,
        origin_unit_id,
    });
}

/// Whether [`sync_corpse_render_entities`] may spawn a new root for this corpse.
pub fn should_spawn_corpse_presentation(
    world: &WorldData,
    corpse_id: CorpseId,
    corpse_index: &CorpseRenderIndex,
    origin_has_pending_death_root: bool,
) -> bool {
    if corpse_index.0.contains_key(&corpse_id) {
        return false;
    }
    if origin_has_pending_death_root {
        return false;
    }
    world
        .corpse_store()
        .get(corpse_id)
        .is_some_and(|record| record.state == CorpseState::Present)
}

/// True when an existing unit render root is transitioning into death for this corpse.
pub fn corpse_origin_has_pending_death_root(
    world: &WorldData,
    origin_unit_id: UnitId,
    unit_render_roots: &Query<&UnitRenderEntity>,
    death_presentations: &Query<&DeathPresentation>,
) -> bool {
    if world.get_unit(origin_unit_id).is_some() {
        return false;
    }
    for marker in unit_render_roots.iter() {
        if marker.unit_id == origin_unit_id {
            return true;
        }
    }
    for presentation in death_presentations.iter() {
        if presentation.origin_unit_id == Some(origin_unit_id) {
            return true;
        }
    }
    false
}

/// Drop index entries whose entity is gone or whose transitional claim lost death presentation.
pub fn release_stale_corpse_presentation_owners(
    corpse_index: &mut CorpseRenderIndex,
    entities: &Query<Entity>,
    claims: &Query<&CorpsePresentationClaim>,
    death_presentations: &Query<&DeathPresentation>,
    corpse_render_entities: &Query<&CorpseRenderEntity>,
) -> Vec<CorpseId> {
    let stale_ids: Vec<CorpseId> = corpse_index
        .0
        .iter()
        .filter_map(|(corpse_id, entity)| {
            if entities.get(*entity).is_err() {
                return Some(*corpse_id);
            }
            if claims.get(*entity).is_ok()
                && death_presentations.get(*entity).is_err()
                && corpse_render_entities.get(*entity).is_err()
            {
                return Some(*corpse_id);
            }
            None
        })
        .collect();
    for corpse_id in &stale_ids {
        corpse_index.0.remove(corpse_id);
    }
    stale_ids
}

/// Count presentation roots tracked for a corpse in the render index.
pub fn corpse_presentation_entity_count(
    corpse_index: &CorpseRenderIndex,
    corpse_id: CorpseId,
) -> usize {
    usize::from(corpse_index.0.contains_key(&corpse_id))
}
