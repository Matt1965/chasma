//! Dev Mode road spline authoring editor.

use bevy::prelude::*;

mod actions;
mod domain;
mod input;
mod overlay;
mod panel;
mod state;
mod transaction;

#[cfg(test)]
mod tests;

pub use actions::{handle_road_editor_buttons, setup_road_editor_state};
pub use input::{handle_road_editor_world_input, update_road_editor_snap_preview};
pub use overlay::draw_road_editor_overlay;
pub use panel::{
    cleanup_road_editor_when_hidden, setup_roads_window_panel, sync_dev_roads_panel_visibility,
    sync_road_editor_action_buttons, sync_road_editor_panel,
};
pub use state::RoadEditorUiState;

pub fn handle_road_editor_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,
    mut editor: ResMut<RoadEditorUiState>,
    mut network: ResMut<crate::world::RoadNetwork>,
) {
    if !dev_state.enabled {
        return;
    }
    actions::handle_road_editor_keyboard(&keyboard, &mut dev_state, &mut editor, &mut network);
}
