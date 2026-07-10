use bevy::prelude::*;

use crate::client::{collect_unit_input_intents, dispatch_client_intents, ClientPipelinePlugin};
use crate::debug::DebugOverlayPlugin;
use crate::simulation::{SimulationPlugin, SimulationSystems};
use crate::ui::GameplayUiPlugin;
use crate::units::input::{BoxSelectDrag, PlayerInteractionSettings, SelectedUnits};
use crate::units::{sync_unit_health_bars, UnitHealthBarState};

use super::box_select_overlay::{setup_box_select_overlay, sync_box_select_overlay};
use super::indicator::{sync_unit_selection_indicators, UnitSelectionIndicatorState};
use super::ownership::LocalPlayerOwnership;
use super::selection_policy::sync_selection_policy_state;
use super::simulation::{apply_death_client_cleanup, flush_simulation_command_trace, tick_unit_movement};

/// Systems for client-local player unit control (ADR-033 U8, ADR-034 U9).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PlayerControlSystems;

/// Owns player-facing unit interaction (selection and move commands).
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPipelinePlugin)
            .add_plugins(SimulationPlugin)
            .add_plugins(DebugOverlayPlugin)
            .add_plugins(GameplayUiPlugin);
        #[cfg(feature = "dev")]
        app.add_plugins(crate::dev::DevModePlugin);
        app.register_type::<PlayerInteractionSettings>()
            .init_resource::<SelectedUnits>()
            .init_resource::<LocalPlayerOwnership>()
            .init_resource::<BoxSelectDrag>()
            .init_resource::<PlayerInteractionSettings>()
            .init_resource::<UnitSelectionIndicatorState>()
            .init_resource::<UnitHealthBarState>()
            .add_systems(Startup, setup_box_select_overlay)
            .add_systems(
                Update,
                tick_unit_movement
                    .in_set(SimulationSystems)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                apply_death_client_cleanup
                    .after(tick_unit_movement)
                    .before(flush_simulation_command_trace)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                flush_simulation_command_trace
                    .after(apply_death_client_cleanup)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                sync_selection_policy_state
                    .after(flush_simulation_command_trace)
                    .before(collect_unit_input_intents)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                collect_unit_input_intents
                    .after(sync_selection_policy_state)
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
                    sync_box_select_overlay,
                    sync_unit_selection_indicators,
                    sync_unit_health_bars,
                )
                    .chain()
                    .after(crate::debug::flush_intent_dispatch_trace)
                    .in_set(PlayerControlSystems),
            );
    }
}
