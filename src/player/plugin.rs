use bevy::prelude::*;

use super::indicator::{sync_unit_selection_indicator, UnitSelectionIndicatorState};
use super::input::handle_player_unit_input;
use super::selection::PlayerUnitSelection;
use super::settings::PlayerInteractionSettings;
use super::simulation::tick_unit_movement;

/// Systems for client-local player unit control (ADR-033 U8).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PlayerControlSystems;

/// Owns player-facing unit interaction (selection and move commands).
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PlayerUnitSelection>()
            .register_type::<PlayerInteractionSettings>()
            .init_resource::<PlayerUnitSelection>()
            .init_resource::<PlayerInteractionSettings>()
            .init_resource::<UnitSelectionIndicatorState>()
            .add_systems(
                Update,
                (
                    tick_unit_movement,
                    handle_player_unit_input,
                    sync_unit_selection_indicator,
                )
                    .chain()
                    .in_set(PlayerControlSystems),
            );
    }
}
