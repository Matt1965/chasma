//! Inventory panel keyboard input (ADR-092 I6).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::client::inventory_intent::{InventoryIntent, InventoryIntentQueue, InventoryOpenMode};
use crate::ui::gameplay::inventory::state::InventoryUiState;
use crate::ui::gameplay::player_hud_state::primary_selected_unit;
use crate::units::input::SelectedUnits;

pub fn collect_inventory_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    selection: Res<SelectedUnits>,
    mut ui: ResMut<InventoryUiState>,
    mut queue: ResMut<InventoryIntentQueue>,
    menu_block: Option<Res<crate::menu::MenuInputBlock>>,
) {
    let _ = &ui;
    if menu_block.is_some_and(|block| block.blocks()) {
        return;
    }
    // Escape is owned by the application menu. Close inventory with UI controls / I toggle.

    if keyboard.just_pressed(KeyCode::KeyI) {
        let Some(unit_id) = primary_selected_unit(&selection) else {
            return;
        };
        if ui.open {
            queue.push(InventoryIntent::Close);
        } else {
            queue.push(InventoryIntent::Open(InventoryOpenMode::UnitOnly {
                unit_id,
            }));
        }
    }
}

pub fn inventory_panel_blocks_world_input(ui: &InventoryUiState) -> bool {
    ui.open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_panel_blocks_world_input() {
        let mut ui = InventoryUiState::default();
        assert!(!inventory_panel_blocks_world_input(&ui));
        ui.open = true;
        assert!(inventory_panel_blocks_world_input(&ui));
    }
}
