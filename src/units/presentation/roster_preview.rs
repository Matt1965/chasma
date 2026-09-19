//! Multi-actor character preview foundation (CG7).

use bevy::prelude::*;

use crate::menu::SquadMemberDraftId;

/// Marks one spawned actor inside a multi-actor preview roster (CG7/CG8).
///
/// Each active [`SquadMemberDraftId`] maps to exactly one preview actor entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct UnitEditorPreviewRosterMember {
    pub draft_member_id: SquadMemberDraftId,
    pub slot_index: usize,
}

/// Compute evenly spaced preview offsets for `count` actors.
pub fn roster_preview_offsets(count: usize, spacing: f32) -> Vec<Vec3> {
    if count == 0 {
        return Vec::new();
    }
    let center = (count.saturating_sub(1) as f32) * 0.5;
    (0..count)
        .map(|index| Vec3::new((index as f32 - center) * spacing, 0.0, 0.0))
        .collect()
}
