//! Squad vs focused preview presentation (visibility + stage layout) for CG8.

use bevy::prelude::*;

use crate::menu::StartingSquadSession;
use crate::units::{
    AnimationPlaybackPending, UnitAnimationRuntime,
    presentation::{
        UnitEditorPreviewRosterMember, UnitPresentationAppearance, roster_stage_layout_position,
    },
};
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

/// Whether a roster preview actor has finished animation initialization and may be shown.
///
/// Reuses the CG3 preview idle pipeline: [`UnitAnimationRuntime`] present and
/// [`AnimationPlaybackPending`] absent once an animation profile exists.
pub fn roster_actor_presentation_ready(
    has_animation_profile: bool,
    has_runtime: bool,
    playback_pending: bool,
) -> bool {
    if !has_animation_profile {
        return true;
    }
    has_runtime && !playback_pending
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
        Option<&UnitAnimationRuntime>,
        Option<&AnimationPlaybackPending>,
    )>,
) {
    let Some(draft) = session.active_draft(&origins) else {
        return;
    };
    let mode = RosterPresentationMode::from_session(&session);

    for (member, appearance, mut visibility, mut transform, runtime, playback_pending) in
        &mut roster
    {
        let Some(draft_member) = draft.member_by_slot(member.slot_index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(definition) = unit_catalog.get(&draft_member.definition_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let mode_visible = roster_member_is_visible(member.slot_index, mode);
        let presentation_ready = roster_actor_presentation_ready(
            definition.animation_profile_id.is_some(),
            runtime.is_some(),
            playback_pending.is_some(),
        );
        let visible = mode_visible && presentation_ready;
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        // Layout is mode-exclusive; position hidden actors so they appear in place once ready.
        if !mode_visible {
            continue;
        }

        let squad_offset =
            roster_stage_layout_position(member.slot_index, draft.members.len());
        transform.translation =
            roster_member_stage_translation(member.slot_index, squad_offset, mode);
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
    fn presentation_ready_without_animation_profile() {
        assert!(roster_actor_presentation_ready(false, false, false));
    }

    #[test]
    fn presentation_ready_requires_idle_runtime_when_profile_exists() {
        assert!(!roster_actor_presentation_ready(true, false, false));
        assert!(!roster_actor_presentation_ready(true, false, true));
        assert!(!roster_actor_presentation_ready(true, true, true));
        assert!(roster_actor_presentation_ready(true, true, false));
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
