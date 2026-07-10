//! Attack-cycle lifetime rules (REVIEW-A2, ADR-056, ADR-058).

use crate::world::unit::{CombatState, UnitId};
use crate::world::{
    validate_attack_target, AttackTargetingPolicy, UnitCatalog, WeaponCatalog, WorldData,
};

use super::range::weapon_for_unit_record;
use super::strike::{CombatStrikeEvent, CombatStrikeReport, CombatStrikeTrace};
use super::targeting::is_unit_alive;
use crate::world::weapon::WeaponDefinitionId;

/// Combat-state target used for authoritative strike targeting.
pub fn combat_engagement_target(combat_state: &CombatState) -> Option<UnitId> {
    match combat_state {
        CombatState::Attacking { target } | CombatState::Chasing { target } => Some(*target),
        CombatState::AttackMoving { target: Some(target), .. } => Some(*target),
        _ => None,
    }
}

/// Whether the unit may hold or advance weapon timing for a combat target.
pub fn is_attack_capable_combat_state(combat_state: &CombatState) -> bool {
    matches!(
        combat_state,
        CombatState::Attacking { .. }
            | CombatState::Chasing { .. }
            | CombatState::AttackMoving {
                target: Some(_),
                ..
            }
    )
}

pub(crate) fn clear_attack_cycle(world: &mut WorldData, unit_id: UnitId) {
    let _ = world.set_unit_attack_cycle(unit_id, None);
}

fn weapon_id_for_attacker(
    world: &WorldData,
    attacker_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> WeaponDefinitionId {
    world
        .get_unit(attacker_id)
        .and_then(|record| weapon_for_unit_record(record, unit_catalog, weapon_catalog).ok())
        .map(|weapon| weapon.id.clone())
        .unwrap_or_else(|| WeaponDefinitionId::new("unknown"))
}

fn push_cycle_trace(
    report: Option<&mut CombatStrikeReport>,
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    event: CombatStrikeEvent,
) {
    let Some(report) = report else {
        return;
    };
    report.push(CombatStrikeTrace {
        attacker_id,
        target_id,
        weapon_id: weapon_id_for_attacker(world, attacker_id, unit_catalog, weapon_catalog),
        event,
    });
}

/// Clear timing state when an order cancels or replaces engagement.
pub fn clear_attack_cycle_for_order_cancel(
    world: &mut WorldData,
    unit_id: UnitId,
    trace: Option<&mut CombatStrikeReport>,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) {
    let Some(record) = world.get_unit(unit_id) else {
        return;
    };
    if record.attack_cycle.is_none() {
        return;
    }
    let trace_target = record
        .attack_cycle
        .as_ref()
        .map(|cycle| cycle.target)
        .or_else(|| combat_engagement_target(&record.combat_state))
        .unwrap_or(unit_id);
    clear_attack_cycle(world, unit_id);
    push_cycle_trace(
        trace,
        world,
        unit_id,
        trace_target,
        unit_catalog,
        weapon_catalog,
        CombatStrikeEvent::AttackCycleClearedOrderCancelled,
    );
}

/// Clear timing state when engagement target is no longer valid.
pub fn clear_attack_cycle_for_invalid_target(
    world: &mut WorldData,
    unit_id: UnitId,
    target: UnitId,
    trace: Option<&mut CombatStrikeReport>,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) {
    if world.get_unit(unit_id).and_then(|r| r.attack_cycle.as_ref()).is_none() {
        return;
    }
    clear_attack_cycle(world, unit_id);
    push_cycle_trace(
        trace,
        world,
        unit_id,
        target,
        unit_catalog,
        weapon_catalog,
        CombatStrikeEvent::AttackCycleClearedInvalidTarget { target },
    );
}

/// Clear timing state when a new attack order selects a different target.
pub fn reset_attack_cycle_for_retarget(
    world: &mut WorldData,
    unit_id: UnitId,
    old_target: UnitId,
    new_target: UnitId,
    trace: Option<&mut CombatStrikeReport>,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) {
    if world
        .get_unit(unit_id)
        .and_then(|record| record.attack_cycle.as_ref())
        .is_none()
    {
        return;
    }
    clear_attack_cycle(world, unit_id);
    push_cycle_trace(
        trace,
        world,
        unit_id,
        new_target,
        unit_catalog,
        weapon_catalog,
        CombatStrikeEvent::AttackCycleResetRetarget {
            old_target,
            new_target,
        },
    );
}

/// Defensive strike gate — returns the authoritative combat target when valid.
pub fn validate_attack_cycle_for_strike(
    world: &WorldData,
    unit_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    targeting_policy: AttackTargetingPolicy,
) -> Option<UnitId> {
    let record = world.get_unit(unit_id)?;
    if !is_unit_alive(record) {
        return None;
    }
    if !is_attack_capable_combat_state(&record.combat_state) {
        return None;
    }
    let target = combat_engagement_target(&record.combat_state)?;
    validate_attack_target(
        world,
        unit_id,
        target,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    )
    .ok()?;
    Some(target)
}

pub fn record_strike_state_mismatch(
    report: &mut CombatStrikeReport,
    world: &WorldData,
    attacker_id: UnitId,
    cycle_target: Option<UnitId>,
    combat_target: Option<UnitId>,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) {
    let trace_target = combat_target.or(cycle_target).unwrap_or(attacker_id);
    push_cycle_trace(
        Some(report),
        world,
        attacker_id,
        trace_target,
        unit_catalog,
        weapon_catalog,
        CombatStrikeEvent::AttackStrikeSkippedStateMismatch {
            cycle_target,
            combat_target,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{
        clear_attack_cycle_for_invalid_target, reset_attack_cycle_for_retarget,
        validate_attack_cycle_for_strike, *,
    };
    use super::super::range::weapon_for_unit_record;
    use crate::world::unit::{AttackCycle, AttackPhase, CombatState};
    use crate::world::{
        create_unit_with_ownership, issue_unit_order, step_all_combat_engagement,
        step_all_combat_strikes, starter_unit_definitions, starter_weapon_definitions, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, CombatStrikeEvent, CombatStrikeReport, DoodadCatalog,
        Heightfield, LocalPosition, NavigationConfig, UnitDefinitionId, UnitOrder, UnitOwnership,
        UnitSource, WeaponCatalog,
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

    fn catalog() -> crate::world::UnitCatalog {
        crate::world::UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
    }

    fn weapons() -> WeaponCatalog {
        WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    fn spawn_player(world: &mut WorldData, catalog: &crate::world::UnitCatalog, x: f32, z: f32) -> UnitId {
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

    fn spawn_hostile(world: &mut WorldData, catalog: &crate::world::UnitCatalog, x: f32, z: f32) -> UnitId {
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

    #[test]
    fn repeated_cleanup_is_idempotent() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle::start_windup(hostile, 0.2)),
            )
            .unwrap();
        let mut report = CombatStrikeReport::default();
        clear_attack_cycle_for_invalid_target(
            &mut world,
            player,
            hostile,
            Some(&mut report),
            &catalog,
            &weapons,
        );
        clear_attack_cycle_for_invalid_target(
            &mut world,
            player,
            hostile,
            Some(&mut report),
            &catalog,
            &weapons,
        );
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert_eq!(report.traces.len(), 1);
    }

    #[test]
    fn peaceful_unit_fails_strike_validation() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world
            .set_unit_combat_state(player, CombatState::Peaceful)
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle::start_windup(hostile, 0.2)),
            )
            .unwrap();
        assert!(validate_attack_cycle_for_strike(
            &world,
            player,
            &catalog,
            &weapons,
            policy(),
        )
        .is_none());
    }

    #[test]
    fn ownership_change_invalidates_strike_validation() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let friendly = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(11.0, 10.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: friendly })
            .unwrap();
        assert!(validate_attack_cycle_for_strike(
            &world,
            player,
            &catalog,
            &weapons,
            policy(),
        )
        .is_none());
    }

    fn step_combat(world: &mut WorldData, catalog: &crate::world::UnitCatalog, delta: f32) -> CombatStrikeReport {
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

    fn issue_attack(
        world: &mut WorldData,
        catalog: &crate::world::UnitCatalog,
        player: UnitId,
        hostile: UnitId,
    ) {
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

    fn hostile_hp(world: &WorldData, hostile: UnitId) -> u32 {
        world.get_unit(hostile).unwrap().vitals.current_hp
    }

    #[test]
    fn retarget_during_windup_does_not_damage_old_target() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hostile_b = spawn_hostile(&mut world, &catalog, 12.0, 10.0);
        let hp_a_before = hostile_hp(&world, hostile_a);
        let hp_b_before = hostile_hp(&world, hostile_b);
        issue_attack(&mut world, &catalog, player, hostile_a);
        let _ = step_combat(&mut world, &catalog, 0.1);
        issue_attack(&mut world, &catalog, player, hostile_b);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        let _ = step_combat(&mut world, &catalog, 0.1);
        let report = step_combat(&mut world, &catalog, 0.1);
        assert_eq!(hostile_hp(&world, hostile_a), hp_a_before);
        assert!(report.traces.iter().any(|trace| {
            trace.target_id == hostile_b
                && matches!(
                    trace.event,
                    CombatStrikeEvent::AttackStrikeApplied { .. }
                )
        }));
        assert!(hostile_hp(&world, hostile_b) < hp_b_before);
    }

    #[test]
    fn same_target_reattack_preserves_cycle_progress() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        let _ = step_combat(&mut world, &catalog, 0.05);
        let remaining_before = world
            .get_unit(player)
            .unwrap()
            .attack_cycle
            .as_ref()
            .unwrap()
            .phase_remaining_seconds;
        issue_attack(&mut world, &catalog, player, hostile);
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
    fn retarget_during_recovery_starts_fresh_cycle() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hostile_b = spawn_hostile(&mut world, &catalog, 12.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile_a);
        let _ = step_combat(&mut world, &catalog, 0.1);
        let timing = super::super::attack_cycle::WeaponTiming::from_weapon(
            weapon_for_unit_record(world.get_unit(player).unwrap(), &catalog, &weapons).unwrap(),
        );
        world
            .set_unit_attack_cycle(
                player,
                Some({
                    let mut cycle = AttackCycle::start_windup(hostile_a, timing.windup_seconds);
                    cycle.begin_recovery(timing.recovery_seconds);
                    cycle
                }),
            )
            .unwrap();
        issue_attack(&mut world, &catalog, player, hostile_b);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        let report = step_combat(&mut world, &catalog, 0.01);
        assert!(report.traces.iter().any(|trace| {
            trace.target_id == hostile_b
                && matches!(trace.event, CombatStrikeEvent::AttackWindupStarted)
        }));
        assert_eq!(
            world
                .get_unit(player)
                .unwrap()
                .attack_cycle
                .as_ref()
                .unwrap()
                .target,
            hostile_b
        );
    }

    #[test]
    fn move_order_cancels_attack_cycle() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        let _ = step_combat(&mut world, &catalog, 0.05);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_some());
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::MoveTo {
                target: pos(40.0, 40.0),
            },
            policy(),
        )
        .unwrap();
        let record = world.get_unit(player).unwrap();
        assert!(record.attack_cycle.is_none());
        assert_eq!(record.combat_state, CombatState::Peaceful);
    }

    #[test]
    fn idle_order_cancels_attack_cycle() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        let _ = step_combat(&mut world, &catalog, 0.05);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Idle,
            policy(),
        )
        .unwrap();
        let record = world.get_unit(player).unwrap();
        assert!(record.attack_cycle.is_none());
        assert_eq!(record.combat_state, CombatState::Peaceful);
    }

    #[test]
    fn peaceful_unit_clears_stale_cycle_on_strike_tick() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world
            .set_unit_combat_state(player, CombatState::Peaceful)
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle::start_windup(hostile, 0.2)),
            )
            .unwrap();
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert!(report.traces.iter().any(|trace| matches!(
            trace.event,
            CombatStrikeEvent::AttackStrikeSkippedStateMismatch { .. }
        )));
    }

    #[test]
    fn target_death_during_windup_clears_cycle() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        let _ = step_combat(&mut world, &catalog, 0.05);
        world.damage_unit(hostile, 999).unwrap();
        let report = step_combat(&mut world, &catalog, 0.1);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                CombatStrikeEvent::AttackStrikeSkippedStateMismatch { .. }
                    | CombatStrikeEvent::AttackCycleClearedInvalidTarget { .. }
                    | CombatStrikeEvent::AttackStrikeMissedInvalidTarget
            )
        }));
    }

    #[test]
    fn invalid_target_clears_attack_cycle_via_engagement() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, player, hostile);
        let _ = step_combat(&mut world, &catalog, 0.05);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_some());
        world.damage_unit(hostile, 999).unwrap();
        let mut report = CombatStrikeReport::default();
        let _ = step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &mut report,
        );
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert_eq!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Peaceful
        );
        assert!(report.traces.iter().any(|trace| matches!(
            trace.event,
            CombatStrikeEvent::AttackCycleClearedInvalidTarget { .. }
        )));
    }

    #[test]
    fn weapon_target_filter_invalidation_clears_cycle() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let neutral = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(11.0, 10.0),
            UnitSource::Authored,
            UnitOwnership::neutral(),
        )
        .unwrap()
        .id;
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: neutral })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle::start_windup(neutral, 0.2)),
            )
            .unwrap();
        let mut report = CombatStrikeReport::default();
        let strike_report = step_all_combat_strikes(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            0.1,
            &mut crate::world::ProjectileReport::default(),
        );
        report.traces.extend(strike_report.traces);
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert!(report.traces.iter().any(|trace| matches!(
            trace.event,
            CombatStrikeEvent::AttackStrikeSkippedStateMismatch { .. }
        )));
    }

    #[test]
    fn strike_target_must_match_combat_state() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hostile_b = spawn_hostile(&mut world, &catalog, 12.0, 10.0);
        let hp_a_before = hostile_hp(&world, hostile_a);
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile_b })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle::start_windup(hostile_a, 0.01)),
            )
            .unwrap();
        let report = step_combat(&mut world, &catalog, 0.05);
        assert_eq!(hostile_hp(&world, hostile_a), hp_a_before);
        assert!(report.traces.iter().any(|trace| matches!(
            trace.event,
            CombatStrikeEvent::AttackStrikeSkippedStateMismatch {
                cycle_target: Some(t),
                combat_target: Some(c),
            } if t == hostile_a && c == hostile_b
        )));
        assert_eq!(
            world
                .get_unit(player)
                .unwrap()
                .attack_cycle
                .as_ref()
                .map(|cycle| cycle.target),
            Some(hostile_b)
        );
    }

    #[test]
    fn retarget_emits_reset_trace() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hostile_b = spawn_hostile(&mut world, &catalog, 12.0, 10.0);
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle::start_windup(hostile_a, 0.2)),
            )
            .unwrap();
        let mut report = CombatStrikeReport::default();
        reset_attack_cycle_for_retarget(
            &mut world,
            player,
            hostile_a,
            hostile_b,
            Some(&mut report),
            &catalog,
            &weapons,
        );
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert!(report.traces.iter().any(|trace| matches!(
            trace.event,
            CombatStrikeEvent::AttackCycleResetRetarget {
                old_target,
                new_target,
            } if old_target == hostile_a && new_target == hostile_b
        )));
    }

    #[test]
    fn attack_move_resumes_after_target_invalidation() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let destination = pos(40.0, 40.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::AttackMove { destination },
            policy(),
        )
        .unwrap();
        world
            .set_unit_combat_state(
                player,
                CombatState::AttackMoving {
                    destination,
                    target: Some(hostile),
                },
            )
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle::start_windup(hostile, 0.2)),
            )
            .unwrap();
        world.damage_unit(hostile, 999).unwrap();
        let mut report = CombatStrikeReport::default();
        let _ = step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &mut report,
        );
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::AttackMoving {
                destination: dest,
                target: None,
            } if dest == destination
        ));
    }
}
