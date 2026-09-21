//! Origin preview actor count and identity invariants (CG8).

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::menu::SquadMemberDraftId;
use crate::units::presentation::{UnitEditorPreviewRosterMember, UnitEditorPreviewUnit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginPreviewInvariantReport {
    pub roster_actor_count: usize,
    pub cg3_actor_count: usize,
    pub visible_roster_count: usize,
    pub unique_draft_member_ids: usize,
    pub duplicate_draft_member_ids: Vec<SquadMemberDraftId>,
}

impl OriginPreviewInvariantReport {
    pub fn validate_expected_roster_count(&self, expected: usize) -> Result<(), String> {
        if self.cg3_actor_count != 0 {
            return Err(format!(
                "expected 0 CG3 preview actors on origin select, found {}",
                self.cg3_actor_count
            ));
        }
        if self.roster_actor_count != expected {
            return Err(format!(
                "expected {expected} roster preview actors, found {}",
                self.roster_actor_count
            ));
        }
        if self.unique_draft_member_ids != expected {
            return Err(format!(
                "expected {expected} unique draft-member ids, found {}",
                self.unique_draft_member_ids
            ));
        }
        if !self.duplicate_draft_member_ids.is_empty() {
            return Err(format!(
                "duplicate draft-member preview actors: {:?}",
                self.duplicate_draft_member_ids
            ));
        }
        Ok(())
    }
}

pub fn collect_origin_preview_invariant_report(world: &mut World) -> OriginPreviewInvariantReport {
    let mut roster = world.query::<(&UnitEditorPreviewRosterMember, &Visibility)>();
    let mut cg3_preview = world.query_filtered::<Entity, With<UnitEditorPreviewUnit>>();

    let mut ids = HashMap::<SquadMemberDraftId, usize>::new();
    let mut visible_roster_count = 0usize;
    let mut roster_actor_count = 0usize;
    for (member, visibility) in roster.iter(world) {
        roster_actor_count += 1;
        if *visibility == Visibility::Visible {
            visible_roster_count += 1;
        }
        *ids.entry(member.draft_member_id).or_default() += 1;
    }
    let duplicate_draft_member_ids = ids
        .iter()
        .filter_map(|(id, count)| if *count > 1 { Some(*id) } else { None })
        .collect();
    OriginPreviewInvariantReport {
        roster_actor_count,
        cg3_actor_count: cg3_preview.iter(world).count(),
        visible_roster_count,
        unique_draft_member_ids: ids.len(),
        duplicate_draft_member_ids,
    }
}

pub fn focused_draft_member_entity(
    roster: &Query<(Entity, &UnitEditorPreviewRosterMember)>,
    draft_member_id: SquadMemberDraftId,
) -> Option<Entity> {
    roster
        .iter()
        .find(|(_, member)| member.draft_member_id == draft_member_id)
        .map(|(entity, _)| entity)
}

pub fn unique_draft_member_ids(
    roster: &Query<&UnitEditorPreviewRosterMember>,
) -> HashSet<SquadMemberDraftId> {
    roster.iter().map(|member| member.draft_member_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_report_detects_cg3_stray_actor() {
        let mut world = World::new();
        world.spawn((
            UnitEditorPreviewRosterMember {
                draft_member_id: SquadMemberDraftId(0),
                slot_index: 0,
            },
            Visibility::Visible,
        ));
        world.spawn((
            UnitEditorPreviewRosterMember {
                draft_member_id: SquadMemberDraftId(1),
                slot_index: 1,
            },
            Visibility::Visible,
        ));
        world.spawn(UnitEditorPreviewUnit);
        let report = collect_origin_preview_invariant_report(&mut world);
        assert_eq!(report.roster_actor_count, 2);
        assert_eq!(report.cg3_actor_count, 1);
        assert!(report.validate_expected_roster_count(2).is_err());
    }
}
