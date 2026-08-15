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
                DebugOverlaySystems, draw_blueprint_debug_overlay, draw_combat_debug_overlay,
                draw_formation_debug_overlay, draw_intent_debug_overlay,
                draw_interaction_debug_overlay, draw_interior_clearance_overlay,
                draw_navigation_debug_overlay, draw_path_debug_overlay,
                draw_runtime_entrance_overlay, draw_selection_debug_overlay,
                draw_steering_debug_overlay, setup_navigation_mask_overlay_assets,
                sync_navigation_mask_meshes,
            };
            use super::overlay_diagnostics::sync_navigation_overlay_diagnostics;
            use super::path_trace::{UnitPathDiagnosticStore, sync_unit_path_diagnostic_store};
            use super::settings::{
                run_debug_blueprint_overlay, run_debug_combat_overlay, run_debug_formation_overlay,
                run_debug_intent_overlay, run_debug_interaction_overlay,
                run_debug_interior_clearance_overlay, run_debug_navigation_overlay,
                run_debug_path_overlay, run_debug_runtime_entrance_overlay,
                run_debug_selection_overlay, run_debug_steering_overlay,
            };

            app.init_resource::<InteractionDebugSnapshot>()
                .init_resource::<super::overlay::NavigationMaskCache>()
                .init_resource::<super::overlay::NavigationMaskDrawStats>()
                .init_resource::<super::overlay::AuthoredBlueprintOverlayTrace>()
                .init_resource::<UnitPathDiagnosticStore>()
                .init_resource::<super::overlay_diagnostics::NavigationOverlayDiagnostics>()
                .add_systems(Startup, setup_navigation_mask_overlay_assets)
                .add_systems(
                    Update,
                    sync_unit_path_diagnostic_store.after(crate::simulation::SimulationSystems),
                )
                .add_systems(
                    Update,
                    (
                        capture_interaction_debug_snapshot
                            .run_if(run_capture_interaction_debug_snapshot),
                        sync_navigation_overlay_diagnostics,
                        draw_intent_debug_overlay.run_if(run_debug_intent_overlay),
                        draw_interaction_debug_overlay.run_if(run_debug_interaction_overlay),
                        draw_path_debug_overlay.run_if(run_debug_path_overlay),
                        draw_navigation_debug_overlay.run_if(run_debug_navigation_overlay),
                        sync_navigation_mask_meshes,
                        draw_blueprint_debug_overlay.run_if(run_debug_blueprint_overlay),
                        draw_interior_clearance_overlay
                            .run_if(run_debug_interior_clearance_overlay),
                        draw_runtime_entrance_overlay.run_if(run_debug_runtime_entrance_overlay),
                        draw_formation_debug_overlay.run_if(run_debug_formation_overlay),
                        draw_steering_debug_overlay.run_if(run_debug_steering_overlay),
                        draw_selection_debug_overlay.run_if(run_debug_selection_overlay),
                        draw_combat_debug_overlay.run_if(run_debug_combat_overlay),
                    )
                        .chain()
                        .after(crate::debug::flush_intent_dispatch_trace)
                        .in_set(DebugOverlaySystems),
                );
        }
    }
}
