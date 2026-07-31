//! Sync inspector focus into animation LOD overrides (A6).

use bevy::prelude::*;

use crate::client::selection::WorldSelectionState;
use crate::units::AnimationPresentationFocus;
use crate::units::input::SelectedUnits;

pub fn sync_animation_presentation_focus(
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    mut focus: ResMut<AnimationPresentationFocus>,
) {
    let inspected = world_selection.primary_unit(&selected_units);
    if focus.inspected_unit == inspected {
        return;
    }
    focus.inspected_unit = inspected;
}
