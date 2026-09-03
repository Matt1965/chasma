//! Client-local input intent pipeline (ADR-038 U-UI2, ADR-041 U-UI5).
//!
//! Input → Intent → Command → Simulation → Presentation

mod building_interaction_dispatch;
pub mod commands;
mod dispatcher;
mod intent;
pub mod inventory_dispatch;
pub mod inventory_intent;
pub mod pipeline;
pub mod selection;

pub use building_interaction_dispatch::{
    OwnedBuildingInteractionOutcome, PendingBuildingPlayerInteraction,
    PendingBuildingPlayerInteractionState, complete_building_player_interaction,
    resolve_player_owned_building_target, supersede_pending_building_interaction_for_selection,
    tick_pending_building_player_interactions, try_complete_pending_building_player_interaction,
    try_dispatch_owned_building_player_interaction,
};
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
pub use selection::{
    ApplyWorldSelectionParams, WorldSelectionCategory, WorldSelectionChange,
    WorldSelectionRevision, WorldSelectionState, WorldSelectionWriteParams, apply_world_selection,
    prune_world_selection,
};
