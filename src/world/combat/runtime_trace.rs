//! Dev-only combat runtime trace (`COMBAT_TRACE` log lines).
//!
//! Event-based tracing for manual combat debugging. Not per-frame.

use crate::world::unit::{UnitId, UnitOrderError};
use crate::world::{UnitCatalog, WeaponCatalog, WorldData};

use super::engagement::{CombatEngagementStatus, CombatEngagementTrace};
use super::range::{RangeCheck, intended_standoff_edge_distance_meters, weapon_for_unit_record};
use super::standoff::standoff_edge_distance_at_position;
use crate::world::WeaponDefinition;

#[cfg(feature = "dev")]
use std::collections::HashMap;
#[cfg(feature = "dev")]
use std::sync::Mutex;

#[cfg(feature = "dev")]
fn log(line: impl std::fmt::Display) {
    crate::logging::write_combat_trace(line.to_string());
}

#[cfg(feature = "dev")]
fn engagement_status_code(status: CombatEngagementStatus) -> u8 {
    match status {
        CombatEngagementStatus::TargetInvalid => 1,
        CombatEngagementStatus::MissingWeapon => 2,
        CombatEngagementStatus::OutOfRangeChasing => 3,
        CombatEngagementStatus::InRangeReady => 4,
        CombatEngagementStatus::TerrainUnavailable => 5,
        CombatEngagementStatus::PathUnavailable => 6,
        CombatEngagementStatus::AttackMoveAcquired => 7,
        CombatEngagementStatus::AttackMoveMoving => 8,
    }
}

#[cfg(feature = "dev")]
use std::sync::OnceLock;

#[cfg(feature = "dev")]
fn engagement_last() -> &'static Mutex<HashMap<u64, u8>> {
    static ENGAGEMENT_LAST: OnceLock<Mutex<HashMap<u64, u8>>> = OnceLock::new();
    ENGAGEMENT_LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

#[inline(always)]
pub fn attack_order_requested(attacker: UnitId, target: UnitId) {
    #[cfg(feature = "dev")]
    log(format!(
        "1 attack_order_requested attacker={} target={}",
        attacker.0, target.0
    ));
}

#[inline(always)]
pub fn attack_order_accepted(attacker: UnitId, target: UnitId) {
    #[cfg(feature = "dev")]
    {
        if let Ok(mut last) = engagement_last().lock() {
            last.remove(&attacker.0);
        }
        log(format!(
            "2 attack_order_accepted attacker={} target={}",
            attacker.0, target.0
        ));
    }
}

#[inline(always)]
pub fn attack_order_rejected(attacker: UnitId, target: UnitId, reason: UnitOrderError) {
    #[cfg(feature = "dev")]
    log(format!(
        "2 attack_order_rejected attacker={} target={} reason={reason}",
        attacker.0, target.0
    ));
}

#[inline(always)]
pub fn combat_state_after_attack_order(world: &WorldData, attacker: UnitId, target: UnitId) {
    #[cfg(feature = "dev")]
    {
        let Some(record) = world.get_unit(attacker) else {
            log(format!(
                "3 combat_state_after_order attacker={} target={} unit=MISSING",
                attacker.0, target.0
            ));
            return;
        };
        log(format!(
            "3 combat_state_after_order attacker={} target={} combat_state={:?} unit_state={:?}",
            attacker.0, target.0, record.combat_state, record.state
        ));
    }
}

#[inline(always)]
pub fn attacker_weapon_definition(
    world: &WorldData,
    attacker: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) {
    #[cfg(feature = "dev")]
    {
        let Some(record) = world.get_unit(attacker) else {
            log(format!(
                "weapon_definition attacker={} unit=MISSING",
                attacker.0
            ));
            return;
        };
        match weapon_for_unit_record(record, unit_catalog, weapon_catalog) {
            Ok(weapon) => log_weapon_definition(attacker, weapon),
            Err(_) => log(format!(
                "weapon_definition attacker={} unit_def={} weapon=MISSING_OR_DISABLED",
                attacker.0,
                record.definition_id.as_str()
            )),
        }
    }
}

#[cfg(feature = "dev")]
fn log_weapon_definition(attacker: UnitId, weapon: &WeaponDefinition) {
    log(format!(
        "weapon_definition attacker={} id={} enabled={} damage={} range_m={} hit_mode={:?}",
        attacker.0,
        weapon.id.as_str(),
        weapon.enabled,
        weapon.damage,
        weapon.range_meters,
        weapon.hit_mode
    ));
}

#[inline(always)]
pub fn engagement(trace: &CombatEngagementTrace) {
    #[cfg(feature = "dev")]
    {
        let code = engagement_status_code(trace.status);
        if let Ok(mut last) = engagement_last().lock() {
            if last.get(&trace.unit_id.0) == Some(&code) {
                return;
            }
            last.insert(trace.unit_id.0, code);
        }
        log(format!(
            "4 engagement unit={} status={:?} target={:?} edge_distance_m={:?} weapon_range_m={:?}",
            trace.unit_id.0,
            trace.status,
            trace.target.map(|id| id.0),
            trace.edge_distance_meters,
            trace.weapon_range_meters
        ));
    }
}

#[inline(always)]
pub fn attack_windup_started(attacker: UnitId, target: UnitId, weapon_id: &str) {
    #[cfg(feature = "dev")]
    log(format!(
        "5 attack_windup_started attacker={} target={} weapon_id={weapon_id}",
        attacker.0, target.0
    ));
}

#[inline(always)]
pub fn strike_reached(attacker: UnitId, target: UnitId, weapon_id: &str, hit_mode: &str) {
    #[cfg(feature = "dev")]
    log(format!(
        "6 strike_reached attacker={} target={} weapon_id={weapon_id} hit_mode={hit_mode}",
        attacker.0, target.0
    ));
}

#[inline(always)]
pub fn attributed_damage_called(attacker: UnitId, victim: UnitId, damage: u32) {
    #[cfg(feature = "dev")]
    log(format!(
        "7 attributed_damage_called attacker={} victim={} damage={damage}",
        attacker.0, victim.0
    ));
}

#[inline(always)]
pub fn victim_hp_before_after(victim: UnitId, before: u32, after: u32) {
    #[cfg(feature = "dev")]
    log(format!(
        "8 victim_hp victim={} before={before} after={after}",
        victim.0
    ));
}

#[inline(always)]
pub fn retaliation_result(victim: UnitId, attacker: UnitId, issued: bool, detail: &str) {
    #[cfg(feature = "dev")]
    log(format!(
        "9 retaliation_result victim={} attacker={} issued={issued} detail={detail}",
        victim.0, attacker.0
    ));
}

#[inline(always)]
pub fn autonomous_desire_decision(
    observer: UnitId,
    target: UnitId,
    decision: super::autonomous_desire::AutonomousDesireDecision,
) {
    #[cfg(feature = "dev")]
    log(format!(
        "autonomous_desire observer={} target={} effective={} threshold={} wants_attack={} dev_override={}",
        observer.0,
        target.0,
        decision.effective_relationship,
        decision.threshold,
        decision.wants_attack,
        decision.dev_override
    ));
}

#[inline(always)]
pub fn chase_standoff_audit(
    world: &WorldData,
    attacker: UnitId,
    target: UnitId,
    check: &RangeCheck,
    standoff: crate::world::WorldPosition,
    target_pos: crate::world::WorldPosition,
) {
    #[cfg(feature = "dev")]
    {
        let Some(attacker_record) = world.get_unit(attacker) else {
            log(format!(
                "chase_standoff attacker={} target={} attacker_record=MISSING",
                attacker.0, target.0
            ));
            return;
        };
        let standoff_edge = standoff_edge_distance_at_position(world, standoff, target_pos, check);
        let current_edge = check.edge_distance_meters;
        let intended_edge = intended_standoff_edge_distance_meters(check.weapon_range_meters);
        let unit_state = format!("{:?}", attacker_record.state);
        log(format!(
            "chase_standoff attacker={} target={} attacker_radius_m={} target_radius_m={} weapon_range_m={} current_center_m={} current_edge_m={} standoff_edge_m={} intended_standoff_edge_m={} unit_state={unit_state}",
            attacker.0,
            target.0,
            check.attacker_radius_meters,
            check.target_radius_meters,
            check.weapon_range_meters,
            check.center_distance_meters,
            current_edge,
            standoff_edge,
            intended_edge,
        ));
    }
}
