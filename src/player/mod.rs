//! Player / gameplay input layer (ADR-033 U8, ADR-034 U9).

mod box_select_overlay;
mod indicator;
mod move_feedback;
mod ownership;
mod plugin;
mod selection_policy;
mod simulation;

pub use move_feedback::MoveCommandFeedback;
pub use ownership::{LocalPlayerOwnership, selection_policy_for_frame};
pub use plugin::{
    DebugPresentationSystems, GameplayPresentationSystems, PlayerControlSystems, PlayerPlugin,
    RuntimeSyncSystems,
};
pub use selection_policy::{SelectionPolicyState, sync_selection_policy_state};
pub use simulation::{
    apply_death_client_cleanup, flush_simulation_command_trace, tick_unit_movement,
};

pub use crate::units::input::{
    BoxSelectDrag, PlayerInteractionSettings, SelectedUnits, TerrainClickResult,
    authoritative_position_at_global_xz, cursor_screen_position, cursor_world_ray,
    pick_unit_along_ray, terrain_click_to_world_position, unit_pick_radius,
    world_position_to_screen,
};

pub use crate::client::{ClientIntent, collect_unit_input_intents, dispatch_client_intents};

/// Back-compat alias for U8 single-select resource name (ADR-033).
pub type PlayerUnitSelection = SelectedUnits;
