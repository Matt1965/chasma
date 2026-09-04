//! Unit Skills panel keyboard input.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::ui::gameplay::build_mode::BuildModeState;
use crate::ui::gameplay::player_hud_state::primary_selected_unit;
use crate::units::input::SelectedUnits;

use super::state::UnitSkillsPanelState;

pub fn collect_unit_skills_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    selection: Res<SelectedUnits>,
    build_mode: Res<BuildModeState>,
    mut panel: ResMut<UnitSkillsPanelState>,
    menu_block: Option<Res<crate::menu::MenuInputBlock>>,
    #[cfg(feature = "dev")] dev_state: Option<Res<crate::dev::DevModeState>>,
) {
    if menu_block.is_some_and(|block| block.blocks()) {
        return;
    }
    if build_mode.search_focused {
        return;
    }
    #[cfg(feature = "dev")]
    if dev_state.is_some_and(|state| state.has_text_focus()) {
        return;
    }

    if !keyboard.just_pressed(KeyCode::KeyU) {
        return;
    }

    if panel.open {
        panel.close();
        return;
    }

    let Some(unit_id) = primary_selected_unit(&selection) else {
        return;
    };
    panel.open_for(unit_id);
}
