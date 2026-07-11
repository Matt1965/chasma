//! Weapon strike resolution and damage application (ADR-058 C5).

use crate::world::unit::{AttackCycle, AttackPhase, CombatState, UnitId, unit_can_execute_actions};
use crate::world::{
    AttackTargetingPolicy, DoodadCatalog, HitMode, NavigationConfig, ProjectileRecord,
    ProjectileReport, UnitCatalog, WeaponCatalog, WorldData, spawn_projectile_from_strike,
    validate_attack_target,
};

use super::attack_cycle::WeaponTiming;
use super::cycle_lifecycle::{
    clear_attack_cycle, combat_engagement_target, is_attack_capable_combat_state,
    record_strike_state_mismatch, validate_attack_cycle_for_strike,
};
use super::range::{is_in_weapon_range, weapon_for_unit_record};
use super::targeting::is_unit_alive;
use crate::world::weapon::WeaponDefinitionId;

/// Combat damage trace events (ADR-058 C5).
#[derive(Debug, Clone, PartialEq)]
pub enum CombatStrikeEvent {
    AttackWindupStarted,
    AttackStrikeApplied {
        damage: f32,
        target_hp_before: u32,
        target_hp_after: u32,
    },
    AttackStrikeMissedInvalidTarget,
    AttackRecoveryStarted,
    AttackCooldownStarted,
    UnsupportedProjectileMode,
    AttackCycleResetRetarget {
        old_target: UnitId,
        new_target: UnitId,
    },
    AttackCycleClearedInvalidTarget {
        target: UnitId,
    },
    AttackCycleClearedOrderCancelled,
    AttackStrikeSkippedStateMismatch {
        cycle_target: Option<UnitId>,
        combat_target: Option<UnitId>,
    },
}

/// One strike-timing trace row.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatStrikeTrace {
    pub attacker_id: UnitId,
    pub target_id: UnitId,
    pub weapon_id: WeaponDefinitionId,
    pub event: CombatStrikeEvent,
}

/// Aggregated strike tick report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatStrikeReport {
    pub traces: Vec<CombatStrikeTrace>,
}

impl CombatStrikeReport {
    pub fn push(&mut self, trace: CombatStrikeTrace) {
        self.traces.push(trace);
    }

    pub fn has_event(&self, event: &CombatStrikeEvent) -> bool {
        self.traces.iter().any(|trace| &trace.event == event)
    }
}

/// Advance weapon timing and apply damage for in-range attackers.
pub fn step_all_combat_strikes(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    delta_seconds: f32,
    projectile_report: &mut ProjectileReport,
) -> CombatStrikeReport {
    let _ = (doodad_catalog, nav_config);
    let unit_ids = world.sorted_unit_ids();
    let mut report = CombatStrikeReport::default();
    for unit_id in unit_ids {
        step_unit_combat_strike(
            world,
            unit_catalog,
            weapon_catalog,
            targeting_policy,
            unit_id,
            delta_seconds,
            &mut report,
            projectile_report,
        );
    }
    report
}

fn step_unit_combat_strike(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    targeting_policy: AttackTargetingPolicy,
    unit_id: UnitId,
    delta_seconds: f32,
    report: &mut CombatStrikeReport,
    projectile_report: &mut ProjectileReport,
) {
    let Some(attacker) = world.get_unit(unit_id).cloned() else {
        return;
    };

    let cycle_target = attacker.attack_cycle.as_ref().map(|cycle| cycle.target);
    let combat_target = combat_engagement_target(&attacker.combat_state);

    if !unit_can_execute_actions(world, unit_id) {
        if attacker.attack_cycle.is_some() {
            clear_attack_cycle(world, unit_id);
        }
        return;
    }

    if !is_unit_alive(&attacker) {
        if attacker.attack_cycle.is_some() {
            clear_attack_cycle(world, unit_id);
        }
        return;
    }

    if !is_attack_capable_combat_state(&attacker.combat_state) {
        if attacker.attack_cycle.is_some() {
            record_strike_state_mismatch(
                report,
                world,
                unit_id,
                cycle_target,
                combat_target,
                unit_catalog,
                weapon_catalog,
            );
            clear_attack_cycle(world, unit_id);
        }
        return;
    }

    let Some(target_id) = validate_attack_cycle_for_strike(
        world,
        unit_id,
        unit_catalog,
        weapon_catalog,
        targeting_policy,
    ) else {
        if let Some(target) = cycle_target.or(combat_target) {
            if attacker.attack_cycle.is_some() {
                record_strike_state_mismatch(
                    report,
                    world,
                    unit_id,
                    cycle_target,
                    combat_target,
                    unit_catalog,
                    weapon_catalog,
                );
                clear_attack_cycle(world, unit_id);
                resume_chase_after_failed_strike(world, unit_id, target);
            }
        }
        return;
    };

    if cycle_target.is_some_and(|cycle_target| cycle_target != target_id) {
        record_strike_state_mismatch(
            report,
            world,
            unit_id,
            cycle_target,
            Some(target_id),
            unit_catalog,
            weapon_catalog,
        );
        clear_attack_cycle(world, unit_id);
    }

    let Some(target) = world.get_unit(target_id).cloned() else {
        clear_attack_cycle(world, unit_id);
        resume_chase_after_failed_strike(world, unit_id, target_id);
        return;
    };

    let Ok(weapon) = weapon_for_unit_record(&attacker, unit_catalog, weapon_catalog) else {
        clear_attack_cycle(world, unit_id);
        return;
    };

    let existing_cycle = world
        .get_unit(unit_id)
        .and_then(|record| record.attack_cycle.clone());

    if existing_cycle.is_none() {
        if !is_in_weapon_range(world, &attacker, &target, unit_catalog, weapon) {
            return;
        }

        if !matches!(
            weapon.hit_mode,
            HitMode::Melee | HitMode::RangedInstant | HitMode::Projectile
        ) {
            return;
        }
    }

    let timing = WeaponTiming::from_weapon(weapon);
    let cycle_needs_start = existing_cycle
        .as_ref()
        .map(|cycle| cycle.target != target_id)
        .unwrap_or(true);

    if cycle_needs_start {
        if !matches!(
            weapon.hit_mode,
            HitMode::Melee | HitMode::RangedInstant | HitMode::Projectile
        ) {
            clear_attack_cycle(world, unit_id);
            return;
        }
        if !is_in_weapon_range(world, &attacker, &target, unit_catalog, weapon) {
            clear_attack_cycle(world, unit_id);
            return;
        }
        let cycle = AttackCycle::start_windup(target_id, timing.windup_seconds);
        set_attack_cycle(world, unit_id, cycle);
        report.push(CombatStrikeTrace {
            attacker_id: unit_id,
            target_id,
            weapon_id: weapon.id.clone(),
            event: CombatStrikeEvent::AttackWindupStarted,
        });
    }

    let mut cycle = world
        .get_unit(unit_id)
        .and_then(|record| record.attack_cycle.clone())
        .expect("cycle started above");

    if delta_seconds <= 0.0 {
        set_attack_cycle(world, unit_id, cycle);
        return;
    }

    advance_attack_cycle(
        world,
        unit_catalog,
        weapon_catalog,
        targeting_policy,
        unit_id,
        target_id,
        weapon,
        &timing,
        &mut cycle,
        delta_seconds,
        report,
        projectile_report,
    );

    if world
        .get_unit(unit_id)
        .and_then(|record| record.attack_cycle.as_ref())
        .is_some()
    {
        set_attack_cycle(world, unit_id, cycle);
    }
}

fn advance_attack_cycle(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    targeting_policy: AttackTargetingPolicy,
    unit_id: UnitId,
    target_id: UnitId,
    weapon: &crate::world::WeaponDefinition,
    timing: &WeaponTiming,
    cycle: &mut AttackCycle,
    mut remaining_delta: f32,
    report: &mut CombatStrikeReport,
    projectile_report: &mut ProjectileReport,
) {
    while remaining_delta > 0.0 {
        if cycle.phase_remaining_seconds > remaining_delta {
            cycle.phase_remaining_seconds -= remaining_delta;
            return;
        }

        remaining_delta -= cycle.phase_remaining_seconds;
        cycle.phase_remaining_seconds = 0.0;

        match cycle.phase {
            AttackPhase::Windup => {
                resolve_strike(
                    world,
                    unit_catalog,
                    weapon_catalog,
                    targeting_policy,
                    unit_id,
                    target_id,
                    weapon,
                    cycle,
                    report,
                    projectile_report,
                );
                if world
                    .get_unit(unit_id)
                    .and_then(|record| record.attack_cycle.as_ref())
                    .is_none()
                {
                    return;
                }
                cycle.begin_recovery(timing.recovery_seconds);
                report.push(CombatStrikeTrace {
                    attacker_id: unit_id,
                    target_id,
                    weapon_id: weapon.id.clone(),
                    event: CombatStrikeEvent::AttackRecoveryStarted,
                });
                if timing.recovery_seconds <= 0.0 {
                    continue;
                }
                cycle.phase_remaining_seconds = timing.recovery_seconds;
            }
            AttackPhase::Recovery => {
                if timing.cooldown_seconds > 0.0 {
                    cycle.begin_cooldown(timing.cooldown_seconds);
                    report.push(CombatStrikeTrace {
                        attacker_id: unit_id,
                        target_id,
                        weapon_id: weapon.id.clone(),
                        event: CombatStrikeEvent::AttackCooldownStarted,
                    });
                    cycle.phase_remaining_seconds = timing.cooldown_seconds;
                } else {
                    cycle.restart_windup(timing.windup_seconds);
                    report.push(CombatStrikeTrace {
                        attacker_id: unit_id,
                        target_id,
                        weapon_id: weapon.id.clone(),
                        event: CombatStrikeEvent::AttackWindupStarted,
                    });
                    cycle.phase_remaining_seconds = timing.windup_seconds;
                }
            }
            AttackPhase::Cooldown => {
                cycle.restart_windup(timing.windup_seconds);
                report.push(CombatStrikeTrace {
                    attacker_id: unit_id,
                    target_id,
                    weapon_id: weapon.id.clone(),
                    event: CombatStrikeEvent::AttackWindupStarted,
                });
                cycle.phase_remaining_seconds = timing.windup_seconds;
            }
            AttackPhase::Strike => {
                cycle.begin_recovery(timing.recovery_seconds);
                cycle.phase_remaining_seconds = timing.recovery_seconds;
            }
        }

        if remaining_delta <= 0.0 {
            return;
        }
    }
}

fn resolve_strike(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    targeting_policy: AttackTargetingPolicy,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon: &crate::world::WeaponDefinition,
    cycle: &mut AttackCycle,
    report: &mut CombatStrikeReport,
    projectile_report: &mut ProjectileReport,
) {
    cycle.phase = AttackPhase::Strike;
    cycle.struck_this_cycle = true;

    if !validate_strike_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    ) {
        report.push(CombatStrikeTrace {
            attacker_id,
            target_id,
            weapon_id: weapon.id.clone(),
            event: CombatStrikeEvent::AttackStrikeMissedInvalidTarget,
        });
        clear_attack_cycle(world, attacker_id);
        resume_chase_after_failed_strike(world, attacker_id, target_id);
        return;
    }

    if weapon.hit_mode == HitMode::Projectile {
        resolve_projectile_strike(
            world,
            attacker_id,
            target_id,
            weapon,
            unit_catalog,
            targeting_policy,
            projectile_report,
        );
        return;
    }

    let hp_before = world
        .get_unit(target_id)
        .map(|record| record.vitals.current_hp)
        .unwrap_or(0);
    let damage = weapon.damage.max(0.0) as u32;
    let vitals = world.damage_unit(target_id, damage).expect("target exists");
    world.record_kill_attribution(target_id, attacker_id, hp_before);
    report.push(CombatStrikeTrace {
        attacker_id,
        target_id,
        weapon_id: weapon.id.clone(),
        event: CombatStrikeEvent::AttackStrikeApplied {
            damage: weapon.damage,
            target_hp_before: hp_before,
            target_hp_after: vitals.current_hp,
        },
    });
}

fn resolve_projectile_strike(
    world: &mut WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon: &crate::world::WeaponDefinition,
    unit_catalog: &UnitCatalog,
    targeting_policy: AttackTargetingPolicy,
    projectile_report: &mut ProjectileReport,
) {
    if weapon.projectile_speed_mps <= 0.0 {
        return;
    }

    if !unit_can_execute_actions(world, attacker_id) {
        return;
    }

    let Some(attacker) = world.get_unit(attacker_id) else {
        return;
    };
    let Some(target) = world.get_unit(target_id) else {
        return;
    };

    if weapon.projectile_key.is_none() {
        bevy::log::warn!(
            "weapon `{}` uses projectile hit mode without projectile_key; simulation continues without visual",
            weapon.id.as_str()
        );
    }

    let source_position = attacker.placement.position;
    let target_position = target.placement.position;
    let launch_snapshot =
        crate::world::ProjectileLaunchSnapshot::capture(attacker, weapon, targeting_policy);
    let projectile_id = world.allocate_projectile_id();
    let record = ProjectileRecord::new_in_flight(
        projectile_id,
        attacker_id,
        target_id,
        weapon.id.clone(),
        weapon.damage,
        weapon.damage_type,
        source_position,
        target_position,
        weapon.projectile_speed_mps,
        launch_snapshot,
    );
    let _ = unit_catalog;
    spawn_projectile_from_strike(world, record, projectile_report);
}

fn validate_strike_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    targeting_policy: AttackTargetingPolicy,
) -> bool {
    if !unit_can_execute_actions(world, attacker_id) {
        return false;
    }
    if validate_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    )
    .is_err()
    {
        return false;
    }
    let Some(attacker) = world.get_unit(attacker_id) else {
        return false;
    };
    let Some(target) = world.get_unit(target_id) else {
        return false;
    };
    if !is_unit_alive(attacker) || !is_unit_alive(target) {
        return false;
    }
    let Ok(weapon) = weapon_for_unit_record(attacker, unit_catalog, weapon_catalog) else {
        return false;
    };
    is_in_weapon_range(world, attacker, target, unit_catalog, weapon)
}

fn resume_chase_after_failed_strike(world: &mut WorldData, attacker_id: UnitId, target_id: UnitId) {
    let combat_state = world
        .get_unit(attacker_id)
        .map(|record| record.combat_state.clone());
    match combat_state {
        Some(CombatState::Attacking { .. }) => {
            let _ = world
                .set_unit_combat_state(attacker_id, CombatState::Chasing { target: target_id });
        }
        Some(CombatState::AttackMoving { destination, .. }) => {
            let _ = world.set_unit_combat_state(
                attacker_id,
                CombatState::AttackMoving {
                    destination,
                    target: Some(target_id),
                },
            );
        }
        _ => {}
    }
}

fn set_attack_cycle(world: &mut WorldData, unit_id: UnitId, cycle: AttackCycle) {
    let _ = world.set_unit_attack_cycle(unit_id, Some(cycle));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{SIMULATION_TICK_SECONDS, SimulationControlState};
    use crate::world::combat::step_all_combat_engagement;
    use crate::world::projectile::{ProjectileEvent, ProjectileReport, step_all_projectiles};
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DamageType, Heightfield, HitMode,
        LocalPosition, TargetFilter, UnitDefinitionId, UnitOrder, UnitOwnership, UnitSource,
        WeaponDefinition, WeaponDefinitionId, create_unit_with_ownership, issue_unit_order,
        starter_unit_definitions, starter_weapon_definitions,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> crate::world::WorldPosition {
        crate::world::WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn catalog() -> UnitCatalog {
        UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
    }

    fn weapons() -> WeaponCatalog {
        WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    fn spawn_player(world: &mut WorldData, catalog: &UnitCatalog, x: f32, z: f32) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id
    }

    fn spawn_hostile(world: &mut WorldData, catalog: &UnitCatalog, x: f32, z: f32) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id
    }

    fn issue_attack(world: &mut WorldData, catalog: &UnitCatalog, player: UnitId, hostile: UnitId) {
        issue_unit_order(
            world,
            catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
    }

    fn step_combat(world: &mut WorldData, catalog: &UnitCatalog, delta: f32) -> CombatStrikeReport {
        let mut projectile = crate::world::ProjectileReport::default();
        let mut strikes = step_all_combat_strikes(
            world,
            catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            delta,
            &mut projectile,
        );
        let _ = step_all_projectiles(world, delta, &[]);
        let _ = step_all_combat_engagement(
            world,
            catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &mut strikes,
        );
        strikes
    }

    fn hostile_hp(world: &WorldData, hostile: UnitId) -> u32 {
        world.get_unit(hostile).unwrap().vitals.current_hp
    }

    fn report_skipped_invalid_strike(report: &CombatStrikeReport) -> bool {
        report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                CombatStrikeEvent::AttackStrikeMissedInvalidTarget
                    | CombatStrikeEvent::AttackStrikeSkippedStateMismatch { .. }
                    | CombatStrikeEvent::AttackCycleClearedInvalidTarget { .. }
            )
        })
    }

    fn report_applied_strike(report: &CombatStrikeReport) -> bool {
        report
            .traces
            .iter()
            .any(|trace| matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. }))
    }

    #[test]
    fn no_damage_during_windup() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = hostile_hp(&world, hostile);
        issue_attack(&mut world, &catalog, player, hostile);
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(report.has_event(&CombatStrikeEvent::AttackWindupStarted));
        assert!(!report.has_event(&CombatStrikeEvent::AttackStrikeApplied {
            damage: 8.0,
            target_hp_before: hp_before,
            target_hp_after: hp_before.saturating_sub(8),
        }));
        assert_eq!(hostile_hp(&world, hostile), hp_before);
    }

    #[test]
    fn damage_applies_once_at_strike() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = hostile_hp(&world, hostile);
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                CombatStrikeEvent::AttackStrikeApplied {
                    damage: 8.0,
                    target_hp_before,
                    target_hp_after,
                } if target_hp_before == hp_before && target_hp_after == hp_before.saturating_sub(8)
            )
        }));
        assert_eq!(hostile_hp(&world, hostile), hp_before.saturating_sub(8));
    }

    #[test]
    fn recovery_prevents_immediate_second_strike() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 40).unwrap();
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(report.has_event(&CombatStrikeEvent::AttackRecoveryStarted));
        let hp_after_first = hostile_hp(&world, hostile);
        let follow_up = step_combat(&mut world, &catalog, 0.05);
        assert!(
            !follow_up.traces.iter().any(|trace| {
                matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. })
            })
        );
        assert_eq!(hostile_hp(&world, hostile), hp_after_first);
    }

    fn step_strikes_only(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        delta: f32,
    ) -> CombatStrikeReport {
        let mut projectile = crate::world::ProjectileReport::default();
        step_all_combat_strikes(
            world,
            catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            delta,
            &mut projectile,
        )
    }

    #[test]
    fn attacks_per_second_controls_repeat_timing() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
            .unwrap();
        let weapon = weapons
            .get(&crate::world::weapon::WeaponDefinitionId::new(
                "weapon_wolf_bite",
            ))
            .unwrap();
        let timing = super::super::attack_cycle::WeaponTiming::from_weapon(weapon);
        assert!((timing.attack_period_seconds - (1.0 / 1.2)).abs() < 0.01);
        assert!(
            timing.windup_seconds + timing.recovery_seconds + timing.cooldown_seconds
                <= timing.attack_period_seconds + f32::EPSILON
        );
        let report = step_strikes_only(&mut world, &catalog, timing.attack_period_seconds);
        assert!(
            report.traces.iter().any(|trace| {
                matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. })
            })
        );
    }

    #[test]
    fn out_of_range_at_strike_prevents_damage() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = hostile_hp(&world, hostile);
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        world.relocate_unit(hostile, pos(40.0, 10.0)).unwrap();
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(report.has_event(&CombatStrikeEvent::AttackStrikeMissedInvalidTarget));
        assert_eq!(hostile_hp(&world, hostile), hp_before);
    }

    #[test]
    fn invalid_target_at_strike_prevents_damage() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        world.damage_unit(hostile, 999).unwrap();
        let hp_before = hostile_hp(&world, hostile);
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(report_skipped_invalid_strike(&report));
        assert!(!report_applied_strike(&report));
        assert_eq!(hostile_hp(&world, hostile), hp_before);
    }

    #[test]
    fn dead_attacker_does_not_damage() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = hostile_hp(&world, hostile);
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        world.damage_unit(player, 999).unwrap();
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(!report_applied_strike(&report));
        assert_eq!(hostile_hp(&world, hostile), hp_before);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
    }

    #[test]
    fn dead_target_does_not_receive_damage() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        world.damage_unit(hostile, 999).unwrap();
        let hp_before = hostile_hp(&world, hostile);
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(report_skipped_invalid_strike(&report));
        assert!(!report_applied_strike(&report));
        assert_eq!(hostile_hp(&world, hostile), hp_before);
    }

    #[test]
    fn damage_clamps_hp_at_zero() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 3).unwrap();
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.2);
        assert_eq!(hostile_hp(&world, hostile), 0);
    }

    #[test]
    fn ranged_instant_applies_damage_in_range() {
        let base = catalog();
        let weapon_id = WeaponDefinitionId::new("weapon_test_bow");
        let weapons = WeaponCatalog::from_definitions(vec![WeaponDefinition::new(
            weapon_id.clone(),
            "Test Bow",
            "Test",
            5.0,
            DamageType::Piercing,
            8.0,
            1.0,
            0.1,
            0.1,
            HitMode::RangedInstant,
            None,
            0.0,
            "attack_bow",
            vec![TargetFilter::Enemies],
            None,
            true,
        )])
        .unwrap();
        let mut wolf = base.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        wolf.default_weapon_id = weapon_id;
        let custom_catalog = UnitCatalog::from_definitions(vec![
            wolf,
            base.get(&UnitDefinitionId::new("bandit")).unwrap().clone(),
        ])
        .unwrap();

        let mut world = flat_world();
        let player = spawn_player(&mut world, &custom_catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &custom_catalog, 19.0, 10.0);
        let hp_before = hostile_hp(&world, hostile);
        issue_unit_order(
            &mut world,
            &custom_catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        step_all_combat_engagement(
            &mut world,
            &custom_catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        let mut projectile = ProjectileReport::default();
        let report = step_all_combat_strikes(
            &mut world,
            &custom_catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            0.1,
            &mut projectile,
        );
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                CombatStrikeEvent::AttackStrikeApplied {
                    damage: 5.0,
                    target_hp_before,
                    target_hp_after,
                } if target_hp_before == hp_before && target_hp_after == hp_before.saturating_sub(5)
            )
        }));
    }

    #[test]
    fn projectile_mode_spawns_projectile_at_strike() {
        let catalog = catalog();
        let mut weapon_defs = starter_weapon_definitions();
        weapon_defs.push(WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_test_proj"),
            "Test Proj",
            "Test",
            5.0,
            DamageType::Piercing,
            15.0,
            1.0,
            0.1,
            0.1,
            HitMode::Projectile,
            Some("arrow".to_string()),
            20.0,
            "attack_proj",
            vec![TargetFilter::Enemies],
            None,
            true,
        ));
        let weapons = WeaponCatalog::from_definitions(weapon_defs).unwrap();
        let mut unit_def = catalog.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        unit_def.default_weapon_id = WeaponDefinitionId::new("weapon_test_proj");
        let custom_catalog = UnitCatalog::from_definitions(vec![
            unit_def,
            catalog
                .get(&UnitDefinitionId::new("bandit"))
                .unwrap()
                .clone(),
        ])
        .unwrap();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &custom_catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &custom_catalog, 14.0, 10.0);
        let hp_before = hostile_hp(&world, hostile);
        issue_unit_order(
            &mut world,
            &custom_catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        step_all_combat_engagement(
            &mut world,
            &custom_catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        let mut projectile = ProjectileReport::default();
        step_all_combat_strikes(
            &mut world,
            &custom_catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            0.2,
            &mut projectile,
        );
        assert!(projectile.has_event(&ProjectileEvent::Spawned));
        assert_eq!(hostile_hp(&world, hostile), hp_before);
        assert_eq!(world.projectiles().count(), 1);
    }

    #[test]
    fn pause_prevents_attack_timer_advancement() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        let remaining_before = world
            .get_unit(player)
            .unwrap()
            .attack_cycle
            .as_ref()
            .unwrap()
            .phase_remaining_seconds;

        let mut control = SimulationControlState {
            paused: true,
            ..Default::default()
        };
        assert!(!control.begin_tick());

        let remaining_after = world
            .get_unit(player)
            .unwrap()
            .attack_cycle
            .as_ref()
            .unwrap()
            .phase_remaining_seconds;
        assert!((remaining_before - remaining_after).abs() < f32::EPSILON);
    }

    #[test]
    fn step_once_advances_exactly_one_tick() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        step_combat(&mut world, &catalog, 0.1);
        let remaining_before = world
            .get_unit(player)
            .unwrap()
            .attack_cycle
            .as_ref()
            .unwrap()
            .phase_remaining_seconds;

        let mut control = SimulationControlState {
            paused: true,
            step_once: true,
            ..Default::default()
        };
        assert!(control.begin_tick());
        let report = step_combat(&mut world, &catalog, SIMULATION_TICK_SECONDS);
        control.complete_tick();
        assert!(!control.should_advance());

        let remaining_after = world
            .get_unit(player)
            .unwrap()
            .attack_cycle
            .as_ref()
            .unwrap()
            .phase_remaining_seconds;
        assert!(
            (remaining_before - remaining_after - SIMULATION_TICK_SECONDS).abs() < 0.001,
            "before={remaining_before} after={remaining_after}"
        );
        assert!(!report.has_event(&CombatStrikeEvent::AttackStrikeApplied {
            damage: 8.0,
            target_hp_before: 0,
            target_hp_after: 0,
        }));
    }

    #[test]
    fn deterministic_repeated_combat_timing() {
        let catalog = catalog();
        let mut world_a = flat_world();
        let mut world_b = flat_world();
        let player_a = spawn_player(&mut world_a, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world_a, &catalog, 11.0, 10.0);
        let player_b = spawn_player(&mut world_b, &catalog, 10.0, 10.0);
        let hostile_b = spawn_hostile(&mut world_b, &catalog, 11.0, 10.0);
        issue_attack(&mut world_a, &catalog, player_a, hostile_a);
        issue_attack(&mut world_b, &catalog, player_b, hostile_b);

        for _ in 0..20 {
            step_combat(&mut world_a, &catalog, SIMULATION_TICK_SECONDS);
            step_combat(&mut world_b, &catalog, SIMULATION_TICK_SECONDS);
        }
        assert_eq!(
            hostile_hp(&world_a, hostile_a),
            hostile_hp(&world_b, hostile_b)
        );
        assert_eq!(
            world_a.get_unit(player_a).unwrap().attack_cycle,
            world_b.get_unit(player_b).unwrap().attack_cycle
        );
    }
}
