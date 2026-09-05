//! Settlement Workforce panel keyboard input.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::ui::gameplay::build_mode::BuildModeState;

use super::state::SettlementWorkforcePanelState;

pub fn collect_settlement_workforce_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    build_mode: Res<BuildModeState>,
    mut panel: ResMut<SettlementWorkforcePanelState>,
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

    if !keyboard.just_pressed(KeyCode::KeyN) {
        return;
    }

    panel.toggle();
}
