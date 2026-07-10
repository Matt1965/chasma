//! Debug visualization and command trace layer (ADR-039 U-UI3).

mod combat_log;
mod boundaries;
mod dispatch_pending;
mod flush;
mod gating;
#[cfg(any(test, feature = "dev"))]
mod interaction_capture;
#[cfg(any(test, feature = "dev"))]
mod interaction_snapshot;
mod inspector_focus;
#[cfg(any(test, feature = "dev"))]
mod overlay;
mod pending;
mod plugin;
mod settings;
mod movement_observability;
mod trace;

pub use inspector_focus::InspectorOverlayFocus;
pub use boundaries::{advance_client_frame_index, ClientBoundaryGuard};
pub use combat_log::{format_trace_entry, is_combat_log_outcome, recent_combat_log_lines};
#[cfg(any(test, feature = "dev"))]
pub use interaction_snapshot::InteractionDebugSnapshot;
pub use plugin::DebugOverlayPlugin;
pub use dispatch_pending::{PendingDispatchTrace, PendingDispatchTraceRecord};
pub use flush::flush_intent_dispatch_trace;
pub use pending::PendingSimulationTrace;
pub use settings::{
    debug_combat_overlay_enabled, debug_interaction_overlay_enabled, debug_path_overlay_enabled,
    DebugOverlayCategory, DebugOverlayConfig, DebugOverlaySettings,
};
pub use movement_observability::{
    blocked_reason_label, MovementBlockObservability,
};
pub use trace::{
    ClientFrameIndex, CommandTraceBuffer, CommandTraceEntry, CommandTraceIntentKind,
    CommandTraceOutcome, IntentDispatchHistory, unit_ids_for_intent,
};
#[cfg(feature = "dev")]
pub use overlay::{
    draw_combat_debug_overlay, draw_formation_debug_overlay, draw_intent_debug_overlay,
    draw_interaction_debug_overlay, draw_path_debug_overlay, draw_selection_debug_overlay,
    draw_steering_debug_overlay, DebugOverlaySystems,
};
