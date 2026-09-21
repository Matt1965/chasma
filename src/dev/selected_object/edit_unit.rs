//! Dev Selected Object → Unit Editor entry (CG3).

use bevy::prelude::*;

use crate::client::selection::WorldSelectionState;
use crate::ui::unit_editor::OpenUnitEditorRequest;
use crate::units::input::SelectedUnits;

use super::panel::{DevSelectedObjectActionButton, SelectedObjectAction};

pub fn handle_selected_object_edit_unit(
    mut requests: MessageWriter<OpenUnitEditorRequest>,
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    buttons: Query<(&Interaction, &DevSelectedObjectActionButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed || button.action != SelectedObjectAction::EditUnit {
            continue;
        }
        if let Some(unit_id) = world_selection.primary_unit(&selected_units) {
            requests.write(OpenUnitEditorRequest(unit_id));
        }
    }
}
