//! Debug visualization and command trace layer (ADR-039 U-UI3).

mod boundaries;
mod dispatch_pending;
mod flush;
mod inspector_focus;
mod overlay;
mod pending;
mod plugin;
mod settings;
mod trace;

pub use inspector_focus::InspectorOverlayFocus;
pub use boundaries::{advance_client_frame_index, ClientBoundaryGuard};
pub use overlay::{
    draw_formation_debug_overlay, draw_intent_debug_overlay, draw_interaction_debug_overlay,
    draw_path_debug_overlay, draw_selection_debug_overlay, draw_steering_debug_overlay,
    DebugOverlaySystems,
};
pub use plugin::DebugOverlayPlugin;
pub use dispatch_pending::{PendingDispatchTrace, PendingDispatchTraceRecord};
pub use flush::flush_intent_dispatch_trace;
pub use pending::PendingSimulationTrace;
pub use settings::{DebugOverlayCategory, DebugOverlayConfig, DebugOverlaySettings};
pub use trace::{
    ClientFrameIndex, CommandTraceBuffer, CommandTraceEntry, CommandTraceIntentKind,
    CommandTraceOutcome, IntentDispatchHistory, unit_ids_for_intent,
};
