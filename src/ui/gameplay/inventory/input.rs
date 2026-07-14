//! Inventory panel keyboard input (ADR-092 I6).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::client::inventory_intent::{InventoryIntent, InventoryIntentQueue, InventoryOpenMode};
use crate::ui::gameplay::inventory::state::InventoryUiState;
use crate::ui::gameplay::player_hud_state::primary_selected_unit;
use crate::units::input::SelectedUnits;
use crate::world::WorldData;

pub fn collect_inventory_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    selection: Res<SelectedUnits>,
    world: Res<WorldData>,
    mut ui: ResMut<InventoryUiState>,
    mut queue: ResMut<InventoryIntentQueue>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        if ui.dragging.take().is_some() {
            return;
        }
        if ui.split_dialog.take().is_some() {
            return;
        }
        if ui.open {
            queue.push(InventoryIntent::Close);
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyI) {
        let Some(unit_id) = primary_selected_unit(&selection) else {
            return;
        };
        queue.push(InventoryIntent::Open(InventoryOpenMode::UnitOnly {
            unit_id,
        }));
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
