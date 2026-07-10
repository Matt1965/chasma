//! Client-local input intent pipeline (ADR-038 U-UI2, ADR-041 U-UI5).
//!
//! Input → Intent → Command → Simulation → Presentation

pub mod commands;
mod dispatcher;
mod intent;
pub mod pipeline;

pub use commands::{
    available_commands_for_selection, build_command_plan, command_availability, command_tooltip,
    resolve_contextual_command, BuiltCommandPlan, CommandAvailability, CommandPaletteEntry,
    CommandResolutionContext, CommandTarget, CommandType, CommandUnavailableReason,
    ContextualCommandIntent, ResolvedCommandFeedback,
};
pub use dispatcher::{
    dispatch_client_intents, IntentDispatchRecord, IntentDispatchReport, IntentDispatchStatus,
};
pub use intent::{ClientInputModifiers, ClientIntent, ClientIntentQueue};
pub use pipeline::{
    collect_unit_input_intents, ClientIntentCollectSystems, ClientIntentDispatchSystems,
    ClientPipelinePlugin, ClientPipelineSystems,
};
