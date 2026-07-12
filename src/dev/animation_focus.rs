//! Sync inspector focus into animation LOD overrides (A6).

use bevy::prelude::*;

use crate::units::AnimationPresentationFocus;

use super::inspector::WorldInspectorState;

pub fn sync_animation_presentation_focus(
    inspector: Res<WorldInspectorState>,
    mut focus: ResMut<AnimationPresentationFocus>,
) {
    if focus.inspected_unit == inspector.selected_unit {
        return;
    }
    focus.inspected_unit = inspector.selected_unit;
}
