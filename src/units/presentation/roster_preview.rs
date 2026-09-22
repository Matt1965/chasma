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

/// Horizontal spacing for origin-select stage layout (not gameplay formation).
pub const ROSTER_STAGE_HORIZONTAL_SPACING: f32 = 1.25;

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

/// Deterministic stage layout for `member_count` actors in preview coordinates.
pub fn roster_stage_layout_offsets(member_count: usize) -> Vec<Vec3> {
    roster_preview_offsets(member_count, ROSTER_STAGE_HORIZONTAL_SPACING)
        .into_iter()
        .enumerate()
        .map(|(index, offset)| {
            let depth = if member_count >= 3 {
                if index % 2 == 0 {
                    -0.25
                } else {
                    0.25
                }
            } else {
                0.0
            };
            Vec3::new(offset.x, 0.0, depth)
        })
        .collect()
}

/// Stage translation for one roster slot in preview coordinates.
pub fn roster_stage_layout_position(slot_index: usize, member_count: usize) -> Vec3 {
    roster_stage_layout_offsets(member_count)
        .get(slot_index)
        .copied()
        .unwrap_or(Vec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_layout_offsets_center_pair_without_overlap() {
        let offsets = roster_stage_layout_offsets(2);
        assert_eq!(offsets.len(), 2);
        assert!(offsets[0].x < 0.0);
        assert!(offsets[1].x > 0.0);
        assert!(offsets[1].x - offsets[0].x >= ROSTER_STAGE_HORIZONTAL_SPACING);
    }

    #[test]
    fn stage_layout_adds_depth_for_three_or_more() {
        let offsets = roster_stage_layout_offsets(3);
        assert_ne!(offsets[0].z, offsets[1].z);
    }
}
