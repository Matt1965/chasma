use bevy::prelude::*;

use crate::client::{
    ClientIntentCollectSystems, ClientIntentDispatchSystems, ClientIntentFlushSystems,
    ClientPipelinePlugin, collect_unit_input_intents, dispatch_client_intents,
};
use crate::debug::DebugOverlayPlugin;
use crate::simulation::{SimulationPlugin, SimulationSystems};
use crate::ui::GameplayUiPlugin;
use crate::ui::gameplay::collect_build_mode_intents;
use crate::ui::gameplay::{GameplayCommandInputSystems, GameplayInputGateSystems};
use crate::units::input::{BoxSelectDrag, PlayerInteractionSettings, SelectedUnits};
use crate::units::{UnitHealthBarState, billboard_unit_health_bars, sync_unit_health_bars};

use super::box_select_overlay::{setup_box_select_overlay, sync_box_select_overlay};
use super::indicator::{UnitSelectionIndicatorState, sync_unit_selection_indicators};
use super::ownership::LocalPlayerOwnership;
use super::selection_policy::sync_selection_policy_state;
use super::simulation::{
    apply_death_client_cleanup, flush_simulation_command_trace, tick_unit_movement,
};
use super::space_view::{ActiveViewedSpace, ViewFollowLock, sync_active_viewed_space};

/// Runtime entity sync from authoritative world data (ADR-065).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct RuntimeSyncSystems;

/// Gameplay HUD and command presentation (ADR-040).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GameplayPresentationSystems;

/// Debug overlay and dev presentation (ADR-039).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DebugPresentationSystems;

/// Systems for client-local player unit control (ADR-033 U8, ADR-034 U9, REVIEW-B5).
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
        #[cfg(feature = "dev")]
        {
            use crate::dev::{DevModeInputSystems, DevModePresentationSystems};
            app.configure_sets(
                Update,
                (
                    DevModeInputSystems,
                    GameplayInputGateSystems,
                    ClientIntentCollectSystems,
                    GameplayCommandInputSystems,
                    ClientIntentDispatchSystems,
                    ClientIntentFlushSystems,
                    GameplayPresentationSystems,
                    DevModePresentationSystems,
                    DebugPresentationSystems,
                )
                    .chain()
                    .in_set(PlayerControlSystems),
            );
        }
        #[cfg(not(feature = "dev"))]
        app.configure_sets(
            Update,
            (
                GameplayInputGateSystems,
                ClientIntentCollectSystems,
                GameplayCommandInputSystems,
                ClientIntentDispatchSystems,
                ClientIntentFlushSystems,
                GameplayPresentationSystems,
                DebugPresentationSystems,
            )
                .chain()
                .in_set(PlayerControlSystems),
        );
        #[cfg(feature = "dev")]
        app.configure_sets(
            Update,
            crate::debug::DebugOverlaySystems.in_set(DebugPresentationSystems),
        );
        app.register_type::<PlayerInteractionSettings>()
            .init_resource::<SelectedUnits>()
            .init_resource::<LocalPlayerOwnership>()
            .init_resource::<BoxSelectDrag>()
            .init_resource::<PlayerInteractionSettings>()
            .init_resource::<UnitSelectionIndicatorState>()
            .init_resource::<UnitHealthBarState>()
            .init_resource::<ActiveViewedSpace>()
            .init_resource::<ViewFollowLock>()
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
                sync_active_viewed_space
                    .after(sync_selection_policy_state)
                    .before(collect_build_mode_intents)
                    .in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                collect_build_mode_intents
                    .after(sync_active_viewed_space)
                    .before(collect_unit_input_intents)
                    .in_set(ClientIntentCollectSystems),
            )
            .add_systems(
                Update,
                collect_unit_input_intents
                    .after(collect_build_mode_intents)
                    .in_set(ClientIntentCollectSystems),
            )
            .add_systems(
                Update,
                dispatch_client_intents
                    .after(collect_unit_input_intents)
                    .in_set(ClientIntentDispatchSystems),
            )
            .add_systems(
                Update,
                crate::debug::flush_intent_dispatch_trace.in_set(ClientIntentFlushSystems),
            )
            .add_systems(
                Update,
                (
                    sync_box_select_overlay,
                    sync_unit_selection_indicators,
                    sync_unit_health_bars,
                    billboard_unit_health_bars,
                )
                    .chain()
                    .after(crate::debug::flush_intent_dispatch_trace)
                    .in_set(PlayerControlSystems),
            );
    }
}
