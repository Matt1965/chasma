//! Debug visualization and command trace layer (ADR-039 U-UI3).

mod boundaries;
mod combat_log;
mod dispatch_pending;
mod flush;
mod gating;
mod inspector_focus;
#[cfg(any(test, feature = "dev"))]
mod interaction_capture;
#[cfg(any(test, feature = "dev"))]
mod interaction_snapshot;
mod movement_observability;
#[cfg(any(test, feature = "dev"))]
mod overlay;
mod pending;
mod plugin;
mod settings;
mod trace;

pub use boundaries::{ClientBoundaryGuard, advance_client_frame_index};
pub use combat_log::{format_trace_entry, is_combat_log_outcome, recent_combat_log_lines};
pub use dispatch_pending::{PendingDispatchTrace, PendingDispatchTraceRecord};
pub use flush::flush_intent_dispatch_trace;
pub use inspector_focus::InspectorOverlayFocus;
#[cfg(any(test, feature = "dev"))]
pub use interaction_snapshot::InteractionDebugSnapshot;
pub use movement_observability::{MovementBlockObservability, blocked_reason_label};
#[cfg(feature = "dev")]
pub use overlay::{
    DebugOverlaySystems, draw_combat_debug_overlay, draw_formation_debug_overlay,
    draw_intent_debug_overlay, draw_interaction_debug_overlay, draw_navigation_debug_overlay,
    draw_path_debug_overlay, draw_selection_debug_overlay, draw_steering_debug_overlay,
};
pub use pending::PendingSimulationTrace;
pub use plugin::DebugOverlayPlugin;
pub use settings::{
    DebugOverlayCategory, DebugOverlayConfig, DebugOverlaySettings, debug_combat_overlay_enabled,
    debug_interaction_overlay_enabled, debug_path_overlay_enabled,
};
pub use trace::{
    ClientFrameIndex, CommandTraceBuffer, CommandTraceEntry, CommandTraceIntentKind,
    CommandTraceOutcome, IntentDispatchHistory, unit_ids_for_intent,
};
