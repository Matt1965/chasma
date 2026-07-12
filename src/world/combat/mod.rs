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
    CombatAiReport, CombatAiScanState, CombatAiSettings, CombatAiTrace, CombatAiTraceOutcome,
    find_auto_acquire_target, step_combat_ai_acquisition,
};
pub use attack_cycle::WeaponTiming;
pub use cycle_lifecycle::{clear_attack_cycle_for_order_cancel, reset_attack_cycle_for_retarget};
pub use engagement::{
    CombatEngagementReport, CombatEngagementStatus, CombatEngagementTrace,
    initial_attack_combat_state, step_all_combat_engagement,
};
pub use range::{RANGE_HYSTERESIS_METERS, RangeCheck, is_in_weapon_range, weapon_for_unit_record};
pub use strike::{
    CombatStrikeEvent, CombatStrikeReport, CombatStrikeTrace, step_all_combat_strikes,
};
pub use targeting::{
    AttackTargetingPolicy, ProjectileImpactRejection, ProjectileLaunchSnapshot,
    classify_unit_target, is_unit_alive, is_valid_attack_target, validate_attack_target,
    validate_projectile_impact_target,
};
