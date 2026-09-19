//! Squad vs focused preview presentation (visibility + stage layout) for CG8.

use bevy::prelude::*;

use crate::menu::StartingSquadSession;
use crate::units::presentation::{UnitEditorPreviewRosterMember, UnitPresentationAppearance};
use crate::world::{OriginCatalog, UnitCatalog, unit_visual_scale};

/// Stage center used for focused member editing.
pub const FOCUS_STAGE_POSITION: Vec3 = Vec3::ZERO;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosterPresentationMode {
    Squad,
    Focused {
        slot_index: usize,
    },
}

impl RosterPresentationMode {
    pub fn from_session(session: &StartingSquadSession) -> Self {
        match session.focused_slot_index() {
            Some(slot_index) => Self::Focused { slot_index },
            None => Self::Squad,
        }
    }
}

/// Whether a roster member should be visible in the current presentation mode.
pub fn roster_member_is_visible(member_slot: usize, mode: RosterPresentationMode) -> bool {
    match mode {
        RosterPresentationMode::Squad => true,
        RosterPresentationMode::Focused { slot_index } => member_slot == slot_index,
    }
}

/// World translation for a roster member in the current presentation mode.
pub fn roster_member_stage_translation(
    member_slot: usize,
    squad_offset: Vec3,
    mode: RosterPresentationMode,
) -> Vec3 {
    match mode {
        RosterPresentationMode::Squad => squad_offset,
        RosterPresentationMode::Focused { slot_index } if member_slot == slot_index => {
            FOCUS_STAGE_POSITION
        }
        RosterPresentationMode::Focused { .. } => squad_offset,
    }
}

pub fn sync_origin_select_preview_presentation(
    session: Res<StartingSquadSession>,
    origins: Res<OriginCatalog>,
    unit_catalog: Res<UnitCatalog>,
    mut roster: Query<(
        &UnitEditorPreviewRosterMember,
        &UnitPresentationAppearance,
        &mut Visibility,
        &mut Transform,
    )>,
) {
    let Some(draft) = session.active_draft(&origins) else {
        return;
    };
    let mode = RosterPresentationMode::from_session(&session);

    for (member, appearance, mut visibility, mut transform) in &mut roster {
        let Some(draft_member) = draft.member_by_slot(member.slot_index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(definition) = unit_catalog.get(&draft_member.definition_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let visible = roster_member_is_visible(member.slot_index, mode);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        // Transform ownership is mode-exclusive: only write layout for visible actors.
        if !visible {
            continue;
        }

        transform.translation = roster_member_stage_translation(
            member.slot_index,
            draft_member.preview_offset,
            mode,
        );
        transform.scale = unit_visual_scale(definition, appearance.appearance.height_scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squad_view_shows_all_members() {
        assert!(roster_member_is_visible(0, RosterPresentationMode::Squad));
        assert!(roster_member_is_visible(1, RosterPresentationMode::Squad));
    }

    #[test]
    fn focused_view_shows_only_selected_member() {
        let mode = RosterPresentationMode::Focused { slot_index: 0 };
        assert!(roster_member_is_visible(0, mode));
        assert!(!roster_member_is_visible(1, mode));
    }

    #[test]
    fn focused_member_moves_to_stage_center() {
        let offset = Vec3::new(-1.5, 0.0, 0.0);
        let mode = RosterPresentationMode::Focused { slot_index: 1 };
        assert_eq!(
            roster_member_stage_translation(1, offset, mode),
            FOCUS_STAGE_POSITION
        );
        assert_eq!(
            roster_member_stage_translation(0, offset, mode),
            offset
        );
    }
}
