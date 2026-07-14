//! Client-local input intent pipeline (ADR-038 U-UI2, ADR-041 U-UI5).
//!
//! Input → Intent → Command → Simulation → Presentation

pub mod commands;
mod dispatcher;
mod intent;
pub mod inventory_dispatch;
pub mod inventory_intent;
pub mod pipeline;

pub use commands::{
    BuiltCommandPlan, CommandAvailability, CommandPaletteEntry, CommandResolutionContext,
    CommandTarget, CommandType, CommandUnavailableReason, ContextualCommandIntent,
    ResolvedCommandFeedback, available_commands_for_selection, build_command_plan,
    command_availability, command_tooltip, resolve_contextual_command,
};
pub use dispatcher::{
    IntentDispatchRecord, IntentDispatchReport, IntentDispatchStatus, dispatch_client_intents,
};
pub use intent::{ClientInputModifiers, ClientIntent, ClientIntentQueue};
pub use inventory_dispatch::{
    dispatch_inventory_intents, try_open_container_inventory, try_open_corpse_inventory,
    try_open_pile_inventory,
};
pub use inventory_intent::{
    InventoryIntent, InventoryIntentQueue, InventoryIntentStatus, InventoryOpenMode,
    entry_revision_for_inventory,
};
pub use pipeline::{
    ClientIntentCollectSystems, ClientIntentDispatchSystems, ClientIntentFlushSystems,
    ClientPipelinePlugin, ClientPipelineSystems, collect_unit_input_intents,
};
