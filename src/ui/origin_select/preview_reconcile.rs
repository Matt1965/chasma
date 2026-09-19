//! Roster preview actor identity reconciliation (CG8).

use std::collections::HashSet;

use bevy::prelude::*;

use crate::menu::SquadMemberDraftId;

/// Despawn roster actors that share a draft-member id (keep the lowest entity id).
pub fn duplicate_roster_actor_entities(
    roster: &[(Entity, SquadMemberDraftId)],
) -> Vec<Entity> {
    let mut keeper: std::collections::HashMap<SquadMemberDraftId, Entity> =
        std::collections::HashMap::new();
    let mut duplicates = Vec::new();
    for (entity, id) in roster {
        match keeper.get(id) {
            Some(kept) => {
                if entity.to_bits() < kept.to_bits() {
                    duplicates.push(*kept);
                    keeper.insert(*id, *entity);
                } else {
                    duplicates.push(*entity);
                }
            }
            None => {
                keeper.insert(*id, *entity);
            }
        }
    }
    duplicates
}

/// Despawn roster actors whose draft member is not in the active draft.
pub fn orphan_roster_actor_entities(
    roster: &[(Entity, SquadMemberDraftId)],
    active_ids: &HashSet<SquadMemberDraftId>,
) -> Vec<Entity> {
    roster
        .iter()
        .filter_map(|(entity, id)| {
            if active_ids.contains(id) {
                None
            } else {
                Some(*entity)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_roster_actor_entities_keeps_lowest_entity_id() {
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let member_id = SquadMemberDraftId(0);
        let roster = if first.to_bits() < second.to_bits() {
            vec![(first, member_id), (second, member_id)]
        } else {
            vec![(second, member_id), (first, member_id)]
        };
        let duplicates = duplicate_roster_actor_entities(&roster);
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0], roster[1].0);
    }

    #[test]
    fn orphan_roster_actor_entities_removes_stale_draft_members() {
        let mut world = World::new();
        let kept = world.spawn_empty().id();
        let orphan = world.spawn_empty().id();
        let active = HashSet::from([SquadMemberDraftId(0)]);
        let orphans = orphan_roster_actor_entities(
            &[(kept, SquadMemberDraftId(0)), (orphan, SquadMemberDraftId(1))],
            &active,
        );
        assert_eq!(orphans, vec![orphan]);
    }
}
