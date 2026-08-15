//! Unit data layer (ADR-027).
//!
//! U1 owns type definitions in [`catalog::UnitCatalog`]. U2 adds authoritative
//! instance records on [`crate::world::WorldData`]. Runtime ECS sync (U3+) and
//! full simulation via [`UnitSimulationState`] subfields are deferred.
//!
//! Obstacle and navigation systems will live under `world/navigation/` or
//! `world/obstacle/` — not under this module.

pub mod animation_profile;
mod attack_cycle;
mod authoring;
mod catalog;
mod combat_state;
mod death;
mod eligibility;
pub(crate) mod entrance_traversal_trace;
mod grounding;
mod id;
pub(crate) mod inside_move_trace;
pub(crate) mod interior_exit_click_trace;
mod inventory;
mod metadata;
mod movement;
mod movement_authority_trace;
pub(crate) mod navigation_membership;
#[cfg(test)]
mod navigation_membership_tests;
mod orders;
mod placement;
mod portal_trace;
pub(crate) mod post_exit_jitter_trace;
mod query;
mod record;
mod removal;
#[cfg(any(test, feature = "dev"))]
mod restore;
mod source;
mod state;
mod store;
#[cfg(feature = "dev")]
pub(crate) mod surface_goal_passability_probe;
mod vitals;

#[cfg(any(test, feature = "dev"))]
pub use animation_profile::starter_definitions as starter_animation_profile_definitions;
pub use animation_profile::{
    AnimationClipKey, AnimationProfile, AnimationProfileCatalog, AnimationProfileCatalogError,
    AnimationProfileId,
};
pub use attack_cycle::{AttackCycle, AttackPhase};
pub use authoring::{
    UnitAuthoringError, create_unit, create_unit_with_inventory, create_unit_with_ownership,
    lookup_unit, move_unit, remove_unit,
};
#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_definitions;
pub use catalog::{
    UnitCatalog, UnitCatalogError, UnitDefinition, UnitDefinitionId, UnitRenderKey,
    UnitWorkCapabilities,
};
pub use combat_state::CombatState;
#[cfg(test)]
pub use death::queue_unit_removal;
pub use death::{
    KillAttribution, RemovalReason, UnitDeathEvent, UnitDeathReport, UnitDeathTrace,
    UnitRemovalEntry, UnitRemovalQueue, step_unit_death_pipeline,
};
pub use eligibility::{unit_can_execute_actions, unit_record_can_execute_actions};
pub use entrance_traversal_trace::EntranceTraversalTrace;
#[cfg(feature = "dev")]
pub use entrance_traversal_trace::{
    maybe_begin_session as maybe_begin_entrance_traversal_session,
    record_interior_first_step as record_entrance_interior_first_step,
    record_membership_update as record_entrance_membership_update,
    record_opening_legality_probe as record_entrance_opening_legality_probe,
    record_pathfinding_probe as record_entrance_pathfinding_probe,
    record_transition_probe as record_entrance_transition_probe,
};
pub use grounding::{UnitGroundingError, ground_unit_position, ground_unit_to_terrain};
pub use id::UnitId;
pub use inside_move_trace::InsideMoveTrace;
#[cfg(feature = "dev")]
pub use inside_move_trace::{
    finish_command_resolution_failure, maybe_begin_session, record_order_issuance,
};
pub use interior_exit_click_trace::InteriorExitClickTrace;
pub use inventory::{
    attach_inventory_on_unit_create, cleanup_unit_inventory_on_delete,
    transfer_unit_inventory_to_corpse, unit_encumbrance_ratio, unit_inventory_weight_grams,
    unit_over_reference_weight_grams, unit_reference_weight_grams, validate_unit_inventory_owner,
};
pub use metadata::UnitMetadata;
pub use movement::{
    BatchUnitMovementReport, BlockedMovementReason, UnitMovementError, UnitMovementReport,
    UnitMovementStepOutcome, UnitMovementStepReport, UnitMovementTrace, UnitSimulationStepReport,
    step_all_unit_movement, step_unit_movement,
};
pub use movement_authority_trace::{
    MovementAuthorityTrace, MovementAuthorityViolation, MovementBlockedAuthorityRecord,
    MovementCommandAuthorityRecord, format_waypoint_spaces, waypoint_space_ids,
};
pub use navigation_membership::{
    infer_navigation_membership_at_position, initialize_surface_units_navigation_membership,
    initialize_unit_navigation_membership, initialize_unit_navigation_membership_if_surface,
};
pub use orders::{
    UnitOrder, UnitOrderError, issue_unit_order, resolve_all_pending_unit_orders,
    resolve_pending_unit_orders,
};
pub use placement::UnitPlacement;
pub use portal_trace::{PortalTransitionEvent, PortalTransitionTrace};
pub use post_exit_jitter_trace::PostExitJitterTrace;
pub use record::UnitRecord;
pub use removal::{UnitRemovalOutcome, finalize_unit_removal};
#[cfg(any(test, feature = "dev"))]
pub use restore::{
    UnitRestoreError, normalize_restored_unit, restore_unit_record, validate_unit_for_restore,
};
pub use source::UnitSource;
pub use state::UnitState;
pub use store::ChunkUnitStore;
pub use vitals::UnitVitals;

#[cfg(test)]
mod waypoint_lookahead_tests;

/// Why [`crate::world::WorldData::insert_unit`] or relocation rejected a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitInsertError {
    /// [`UnitRecord::placement`] chunk does not match the target [`crate::world::ChunkId`].
    ChunkPlacementMismatch,
    /// No unit with the given id exists in world data.
    UnitNotFound,
}

/// Future full simulation envelope (U3+). Not stored separately in U2; [`UnitState`]
/// on [`UnitRecord`] is the minimal placeholder until orders, combat, and AI arrive.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitSimulationState;
