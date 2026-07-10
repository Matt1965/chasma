//! Debug overlay plugin registration (REVIEW-A6).

use bevy::prelude::*;

use crate::player::PlayerControlSystems;

use super::boundaries::advance_client_frame_index;
use super::dispatch_pending::PendingDispatchTrace;
use super::pending::PendingSimulationTrace;
use super::settings::DebugOverlaySettings;
use super::trace::{ClientFrameIndex, CommandTraceBuffer, IntentDispatchHistory};

/// Registers command trace resources and (dev-only) debug overlay presentation systems.
pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandTraceBuffer>()
            .init_resource::<ClientFrameIndex>()
            .init_resource::<IntentDispatchHistory>()
            .init_resource::<DebugOverlaySettings>()
            .init_resource::<super::inspector_focus::InspectorOverlayFocus>()
            .init_resource::<PendingDispatchTrace>()
            .init_resource::<PendingSimulationTrace>()
            .init_resource::<super::boundaries::ClientBoundaryGuard>()
            .register_type::<DebugOverlaySettings>()
            .add_systems(
                Update,
                advance_client_frame_index
                    .before(crate::simulation::SimulationSystems)
                    .in_set(PlayerControlSystems),
            );

        #[cfg(feature = "dev")]
        {
            use super::interaction_capture::{
                capture_interaction_debug_snapshot, run_capture_interaction_debug_snapshot,
            };
            use super::interaction_snapshot::InteractionDebugSnapshot;
            use super::overlay::{
                draw_combat_debug_overlay, draw_formation_debug_overlay, draw_intent_debug_overlay,
                draw_interaction_debug_overlay, draw_path_debug_overlay, draw_selection_debug_overlay,
                draw_steering_debug_overlay, DebugOverlaySystems,
            };
            use super::settings::{
                run_debug_combat_overlay, run_debug_formation_overlay, run_debug_intent_overlay,
                run_debug_interaction_overlay, run_debug_path_overlay, run_debug_selection_overlay,
                run_debug_steering_overlay,
            };

            app.init_resource::<InteractionDebugSnapshot>()
                .add_systems(
                    Update,
                    (
                        capture_interaction_debug_snapshot
                            .run_if(run_capture_interaction_debug_snapshot),
                        draw_intent_debug_overlay.run_if(run_debug_intent_overlay),
                        draw_interaction_debug_overlay.run_if(run_debug_interaction_overlay),
                        draw_path_debug_overlay.run_if(run_debug_path_overlay),
                        draw_formation_debug_overlay.run_if(run_debug_formation_overlay),
                        draw_steering_debug_overlay.run_if(run_debug_steering_overlay),
                        draw_selection_debug_overlay.run_if(run_debug_selection_overlay),
                        draw_combat_debug_overlay.run_if(run_debug_combat_overlay),
                    )
                        .chain()
                        .after(crate::debug::flush_intent_dispatch_trace)
                        .in_set(DebugOverlaySystems)
                        .in_set(PlayerControlSystems),
                );
        }
    }
}
