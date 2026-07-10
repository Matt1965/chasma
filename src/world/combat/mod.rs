//! Combat targeting, range, chase, and strike resolution (ADR-056 C3, ADR-057 C4, ADR-058 C5).
//!
//! Authoritative combat intent lives on [`crate::world::UnitRecord`] via
//! [`crate::world::CombatState`]. C5 adds weapon timing and damage application.

mod ai;
mod attack_cycle;
mod cycle_lifecycle;
mod engagement;
mod range;
mod standoff;
mod strike;
mod targeting;
mod tick_order;

pub use ai::{
    find_auto_acquire_target, step_combat_ai_acquisition, unit_eligible_for_auto_acquire,
    CombatAiReport, CombatAiScanState, CombatAiSettings, CombatAiTrace, CombatAiTraceOutcome,
};
pub use cycle_lifecycle::{
    clear_attack_cycle_for_invalid_target, clear_attack_cycle_for_order_cancel,
    combat_engagement_target, is_attack_capable_combat_state, reset_attack_cycle_for_retarget,
    validate_attack_cycle_for_strike,
};
pub use engagement::{
    initial_attack_combat_state, scan_attack_move_target, step_all_combat_engagement,
    ATTACK_MOVE_SCAN_RADIUS_METERS, CombatEngagementReport, CombatEngagementStatus,
    CombatEngagementTrace,
};
pub use range::{
    center_distance_meters, collision_radius_for_record, edge_distance_meters,
    is_in_weapon_range, is_outside_weapon_range_with_hysteresis, measure_weapon_range,
    range_check_for_units, range_status_from_check, weapon_for_unit_record, RangeCheck,
    RangeStatus, RANGE_HYSTERESIS_METERS,
};
pub use standoff::{
    compute_standoff_destination, standoff_center_distance_matches_weapon_range, StandoffError,
};
pub use strike::{
    step_all_combat_strikes, CombatStrikeEvent, CombatStrikeReport, CombatStrikeTrace,
};
pub use targeting::{
    classify_unit_target, is_unit_alive, is_valid_attack_target, validate_attack_target,
    validate_projectile_impact_target, AttackTargetingPolicy, ProjectileImpactRejection,
    ProjectileLaunchSnapshot,
};
