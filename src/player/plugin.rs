use bevy::prelude::*;

use crate::client::{collect_unit_input_intents, dispatch_client_intents, ClientPipelinePlugin};
use crate::debug::{
    draw_formation_debug_overlay, draw_intent_debug_overlay, draw_interaction_debug_overlay,
    draw_path_debug_overlay, draw_selection_debug_overlay, draw_steering_debug_overlay,
    DebugOverlayPlugin,
};
use crate::ui::GameplayUiPlugin;
use crate::units::input::{BoxSelectDrag, PlayerInteractionSettings, SelectedUnits};

use super::box_select_overlay::{setup_box_select_overlay, sync_box_select_overlay};
use super::indicator::{sync_unit_selection_indicators, UnitSelectionIndicatorState};
use super::simulation::{flush_simulation_command_trace, tick_unit_movement};

/// Systems for client-local player unit control (ADR-033 U8, ADR-034 U9).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PlayerControlSystems;

/// Owns player-facing unit interaction (selection and move commands).
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPipelinePlugin)
            .add_plugins(DebugOverlayPlugin)
            .add_plugins(GameplayUiPlugin)
            .register_type::<PlayerInteractionSettings>()
            .init_resource::<SelectedUnits>()
            .init_resource::<BoxSelectDrag>()
            .init_resource::<PlayerInteractionSettings>()
            .init_resource::<UnitSelectionIndicatorState>()
            .add_systems(Startup, setup_box_select_overlay)
            .add_systems(
                Update,
                tick_unit_movement.in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                flush_simulation_command_trace
                    .after(tick_unit_movement)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                collect_unit_input_intents
                    .after(flush_simulation_command_trace)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                dispatch_client_intents
                    .after(collect_unit_input_intents)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                crate::debug::flush_intent_dispatch_trace
                    .after(dispatch_client_intents)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                (
                    draw_intent_debug_overlay,
                    draw_interaction_debug_overlay,
                    draw_path_debug_overlay,
                    draw_formation_debug_overlay,
                    draw_steering_debug_overlay,
                    draw_selection_debug_overlay,
                )
                    .chain()
                    .after(crate::debug::flush_intent_dispatch_trace)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                (
                    sync_box_select_overlay,
                    sync_unit_selection_indicators,
                )
                    .chain()
                    .after(draw_selection_debug_overlay)
                    .in_set(PlayerControlSystems),
            );
    }
}
