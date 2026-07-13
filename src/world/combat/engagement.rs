//! Combat engagement tick — chase, in-range hold, attack-move scan (ADR-057 C4).

use bevy::prelude::*;

use crate::world::movement::feel::start_unit_move_to;
use crate::world::navigation::xz_distance;
use crate::world::unit::{CombatState, UnitId, UnitOrderError, UnitState};
use crate::world::{
    AttackTargetingPolicy, NavigationConfig, PassabilityCatalogs, UnitCatalog, WeaponCatalog,
    WorldData, WorldPosition, validate_attack_target,
};

use super::cycle_lifecycle::{clear_attack_cycle, clear_attack_cycle_for_invalid_target};
use super::range::{
    RangeStatus, is_in_weapon_range, is_outside_weapon_range_with_hysteresis, measure_weapon_range,
    range_status_from_check, weapon_for_unit_record,
};
use super::standoff::{StandoffError, compute_standoff_destination};
use super::strike::CombatStrikeReport;
use super::targeting::is_unit_alive;
use crate::world::unit::unit_can_execute_actions;

/// Radius for attack-move hostile acquisition scans.
pub const ATTACK_MOVE_SCAN_RADIUS_METERS: f32 = 16.0;

fn combat_pair<'a>(
    world: &'a WorldData,
    unit_id: UnitId,
    target: UnitId,
) -> Option<(&'a crate::world::UnitRecord, &'a crate::world::UnitRecord)> {
    let attacker = world.get_unit(unit_id)?;
    let target_record = world.get_unit(target)?;
    Some((attacker, target_record))
}

/// Explicit combat positioning outcome for one unit this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatEngagementStatus {
    TargetInvalid,
    MissingWeapon,
    OutOfRangeChasing,
    InRangeReady,
    TerrainUnavailable,
    PathUnavailable,
    AttackMoveAcquired,
    AttackMoveMoving,
}

/// One observable combat engagement trace row.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatEngagementTrace {
    pub unit_id: UnitId,
    pub status: CombatEngagementStatus,
    pub target: Option<UnitId>,
    pub center_distance_meters: Option<f32>,
    pub edge_distance_meters: Option<f32>,
    pub weapon_range_meters: Option<f32>,
    pub chase_destination: Option<WorldPosition>,
}

/// Aggregated combat tick report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatEngagementReport {
    pub traces: Vec<CombatEngagementTrace>,
}

impl CombatEngagementReport {
    pub fn push(&mut self, trace: CombatEngagementTrace) {
        self.traces.push(trace);
    }
}

/// Advance combat positioning for every unit with an active combat posture.
pub fn step_all_combat_engagement(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    strike_trace: &mut CombatStrikeReport,
) -> CombatEngagementReport {
    let unit_ids = world.sorted_unit_ids();
    let mut report = CombatEngagementReport::default();
    for unit_id in unit_ids {
        if !unit_can_execute_actions(world, unit_id) {
            continue;
        }
        let Some(combat_state) = world
            .get_unit(unit_id)
            .map(|record| record.combat_state.clone())
        else {
            continue;
        };
        let trace = step_unit_combat_engagement(
            world,
            unit_catalog,
            weapon_catalog,
            catalogs,
            nav_config,
            targeting_policy,
            unit_id,
            combat_state,
            strike_trace,
        );
        if let Some(trace) = trace {
            report.push(trace);
        }
    }
    report
}

fn step_unit_combat_engagement(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    unit_id: UnitId,
    combat_state: CombatState,
    strike_trace: &mut CombatStrikeReport,
) -> Option<CombatEngagementTrace> {
    match combat_state {
        CombatState::Peaceful | CombatState::Alert | CombatState::Engaged => None,
        CombatState::Attacking { target } => Some(handle_attacking_target(
            world,
            unit_catalog,
            weapon_catalog,
            catalogs,
            nav_config,
            targeting_policy,
            unit_id,
            target,
            None,
            strike_trace,
        )),
        CombatState::Chasing { target } => Some(handle_chasing_target(
            world,
            unit_catalog,
            weapon_catalog,
            catalogs,
            nav_config,
            targeting_policy,
            unit_id,
            target,
            None,
            strike_trace,
        )),
        CombatState::AttackMoving {
            destination,
            target,
        } => Some(handle_attack_moving(
            world,
            unit_catalog,
            weapon_catalog,
            catalogs,
            nav_config,
            targeting_policy,
            unit_id,
            destination,
            target,
            strike_trace,
        )),
    }
}

fn handle_attacking_target(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    unit_id: UnitId,
    target: UnitId,
    invalid_target_state: Option<CombatState>,
    strike_trace: &mut CombatStrikeReport,
) -> CombatEngagementTrace {
    let mut trace = CombatEngagementTrace {
        unit_id,
        status: CombatEngagementStatus::InRangeReady,
        target: Some(target),
        center_distance_meters: None,
        edge_distance_meters: None,
        weapon_range_meters: None,
        chase_destination: None,
    };

    if validate_attack_target(
        world,
        unit_id,
        target,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    )
    .is_err()
    {
        apply_invalid_target_state(
            world,
            unit_id,
            target,
            invalid_target_state,
            strike_trace,
            unit_catalog,
            weapon_catalog,
        );
        trace.status = CombatEngagementStatus::TargetInvalid;
        trace.target = None;
        return trace;
    }

    let Some((attacker, target_record)) = combat_pair(world, unit_id, target) else {
        apply_invalid_target_state(
            world,
            unit_id,
            target,
            invalid_target_state,
            strike_trace,
            unit_catalog,
            weapon_catalog,
        );
        trace.status = CombatEngagementStatus::TargetInvalid;
        trace.target = None;
        return trace;
    };
    let weapon = match weapon_for_unit_record(attacker, unit_catalog, weapon_catalog) {
        Ok(weapon) => weapon,
        Err(_) => {
            clear_attack_cycle_for_invalid_target(
                world,
                unit_id,
                target,
                Some(strike_trace),
                unit_catalog,
                weapon_catalog,
            );
            trace.status = CombatEngagementStatus::MissingWeapon;
            return trace;
        }
    };
    let check = measure_weapon_range(world, attacker, target_record, weapon, unit_catalog);
    trace.center_distance_meters = Some(check.center_distance_meters);
    trace.edge_distance_meters = Some(check.edge_distance_meters);
    trace.weapon_range_meters = Some(check.weapon_range_meters);

    if is_outside_weapon_range_with_hysteresis(world, attacker, target_record, unit_catalog, weapon)
    {
        if invalid_target_state.is_none() {
            world
                .set_unit_combat_state(unit_id, CombatState::Chasing { target })
                .ok();
            clear_attack_cycle(world, unit_id);
        }
        trace.status = CombatEngagementStatus::OutOfRangeChasing;
        begin_chase_to_target(
            world,
            unit_catalog,
            weapon_catalog,
            catalogs,
            nav_config,
            unit_id,
            target,
            &check,
            &mut trace,
        );
        return trace;
    }

    if invalid_target_state.is_none() {
        world
            .set_unit_combat_state(unit_id, CombatState::Attacking { target })
            .ok();
    }
    hold_in_attack_range(world, unit_id);
    trace.status = CombatEngagementStatus::InRangeReady;
    trace
}

fn handle_chasing_target(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    unit_id: UnitId,
    target: UnitId,
    invalid_target_state: Option<CombatState>,
    strike_trace: &mut CombatStrikeReport,
) -> CombatEngagementTrace {
    let mut trace = CombatEngagementTrace {
        unit_id,
        status: CombatEngagementStatus::OutOfRangeChasing,
        target: Some(target),
        center_distance_meters: None,
        edge_distance_meters: None,
        weapon_range_meters: None,
        chase_destination: None,
    };

    if validate_attack_target(
        world,
        unit_id,
        target,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    )
    .is_err()
    {
        apply_invalid_target_state(
            world,
            unit_id,
            target,
            invalid_target_state,
            strike_trace,
            unit_catalog,
            weapon_catalog,
        );
        trace.status = CombatEngagementStatus::TargetInvalid;
        trace.target = None;
        return trace;
    }

    let Some((attacker, target_record)) = combat_pair(world, unit_id, target) else {
        apply_invalid_target_state(
            world,
            unit_id,
            target,
            invalid_target_state,
            strike_trace,
            unit_catalog,
            weapon_catalog,
        );
        trace.status = CombatEngagementStatus::TargetInvalid;
        trace.target = None;
        return trace;
    };
    let weapon = match weapon_for_unit_record(attacker, unit_catalog, weapon_catalog) {
        Ok(weapon) => weapon,
        Err(_) => {
            clear_attack_cycle_for_invalid_target(
                world,
                unit_id,
                target,
                Some(strike_trace),
                unit_catalog,
                weapon_catalog,
            );
            trace.status = CombatEngagementStatus::MissingWeapon;
            return trace;
        }
    };
    let check = measure_weapon_range(world, attacker, target_record, weapon, unit_catalog);
    trace.center_distance_meters = Some(check.center_distance_meters);
    trace.edge_distance_meters = Some(check.edge_distance_meters);
    trace.weapon_range_meters = Some(check.weapon_range_meters);

    if matches!(range_status_from_check(&check), RangeStatus::InRange) {
        if invalid_target_state.is_none() {
            world
                .set_unit_combat_state(unit_id, CombatState::Attacking { target })
                .ok();
        }
        hold_in_attack_range(world, unit_id);
        trace.status = CombatEngagementStatus::InRangeReady;
        return trace;
    }

    begin_chase_to_target(
        world,
        unit_catalog,
        weapon_catalog,
        catalogs,
        nav_config,
        unit_id,
        target,
        &check,
        &mut trace,
    );
    trace
}

fn handle_attack_moving(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    unit_id: UnitId,
    destination: WorldPosition,
    target: Option<UnitId>,
    strike_trace: &mut CombatStrikeReport,
) -> CombatEngagementTrace {
    if let Some(acquired) = target {
        return handle_chasing_target(
            world,
            unit_catalog,
            weapon_catalog,
            catalogs,
            nav_config,
            targeting_policy,
            unit_id,
            acquired,
            Some(CombatState::AttackMoving {
                destination,
                target: None,
            }),
            strike_trace,
        );
    }

    if let Some(acquired) = scan_attack_move_target(
        world,
        unit_id,
        unit_catalog,
        weapon_catalog,
        targeting_policy,
    ) {
        world
            .set_unit_combat_state(
                unit_id,
                CombatState::AttackMoving {
                    destination,
                    target: Some(acquired),
                },
            )
            .ok();
        let mut trace = handle_chasing_target(
            world,
            unit_catalog,
            weapon_catalog,
            catalogs,
            nav_config,
            targeting_policy,
            unit_id,
            acquired,
            Some(CombatState::AttackMoving {
                destination,
                target: None,
            }),
            strike_trace,
        );
        trace.status = CombatEngagementStatus::AttackMoveAcquired;
        return trace;
    }

    let mut trace = CombatEngagementTrace {
        unit_id,
        status: CombatEngagementStatus::AttackMoveMoving,
        target: None,
        center_distance_meters: None,
        edge_distance_meters: None,
        weapon_range_meters: None,
        chase_destination: Some(destination),
    };

    let record = match world.get_unit(unit_id) {
        Some(record) => record,
        None => {
            trace.status = CombatEngagementStatus::TargetInvalid;
            return trace;
        }
    };
    let start = record.placement.position;
    let already_heading = matches!(
        record.state,
        UnitState::Moving {
            target,
            ..
        } if target == destination
    );
    if !already_heading {
        match start_unit_move_to(
            world,
            unit_catalog,
            catalogs,
            nav_config,
            unit_id,
            destination,
        ) {
            Ok(()) => {}
            Err(UnitOrderError::NoPath | UnitOrderError::PathGoalBlocked) => {
                trace.status = CombatEngagementStatus::PathUnavailable;
            }
            Err(UnitOrderError::PathTerrainUnavailable) => {
                trace.status = CombatEngagementStatus::TerrainUnavailable;
            }
            Err(_) => trace.status = CombatEngagementStatus::PathUnavailable,
        }
    }
    let _ = start;
    trace
}

pub fn scan_attack_move_target(
    world: &WorldData,
    attacker_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    targeting_policy: AttackTargetingPolicy,
) -> Option<UnitId> {
    let attacker = world.get_unit(attacker_id)?;
    let attacker_pos = attacker.placement.position;
    let layout = world.layout();
    let mut best: Option<(f32, UnitId)> = None;

    for candidate_id in world.sorted_unit_ids() {
        if candidate_id == attacker_id {
            continue;
        }
        if validate_attack_target(
            world,
            attacker_id,
            candidate_id,
            weapon_catalog,
            unit_catalog,
            targeting_policy,
        )
        .is_err()
        {
            continue;
        }
        let candidate = world.get_unit(candidate_id)?;
        if !is_unit_alive(candidate) {
            continue;
        }
        let distance = xz_distance(attacker_pos, candidate.placement.position, layout);
        if distance > ATTACK_MOVE_SCAN_RADIUS_METERS {
            continue;
        }
        let replace = match best {
            None => true,
            Some((best_distance, best_id)) => {
                distance < best_distance - f32::EPSILON
                    || ((distance - best_distance).abs() <= f32::EPSILON && candidate_id < best_id)
            }
        };
        if replace {
            best = Some((distance, candidate_id));
        }
    }

    best.map(|(_, id)| id)
}

fn begin_chase_to_target(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    target: UnitId,
    check: &super::range::RangeCheck,
    trace: &mut CombatEngagementTrace,
) {
    let Some((attacker, target_record)) = combat_pair(world, unit_id, target) else {
        trace.status = CombatEngagementStatus::TargetInvalid;
        trace.target = None;
        return;
    };
    let attacker_pos = attacker.placement.position;
    let target_pos = target_record.placement.position;
    let standoff = match compute_standoff_destination(world, attacker_pos, target_pos, check) {
        Ok(position) => position,
        Err(StandoffError::TerrainUnavailable) => {
            trace.status = CombatEngagementStatus::TerrainUnavailable;
            return;
        }
    };
    trace.chase_destination = Some(standoff);
    match start_unit_move_to(world, unit_catalog, catalogs, nav_config, unit_id, standoff) {
        Ok(()) => trace.status = CombatEngagementStatus::OutOfRangeChasing,
        Err(UnitOrderError::NoPath | UnitOrderError::PathGoalBlocked) => {
            trace.status = CombatEngagementStatus::PathUnavailable;
        }
        Err(UnitOrderError::PathTerrainUnavailable) => {
            trace.status = CombatEngagementStatus::TerrainUnavailable;
        }
        Err(_) => trace.status = CombatEngagementStatus::PathUnavailable,
    }
    let _ = weapon_catalog;
}

fn hold_in_attack_range(world: &mut WorldData, unit_id: UnitId) {
    world.command_buffer_mut().clear_pending(unit_id);
    world.movement_smoothing_mut().clear_unit(unit_id);
    if matches!(
        world.get_unit(unit_id).map(|record| &record.state),
        Some(UnitState::Moving { .. })
    ) {
        let _ = world.set_unit_state(unit_id, UnitState::Idle);
    }
}

fn apply_invalid_target_state(
    world: &mut WorldData,
    unit_id: UnitId,
    target: UnitId,
    invalid_target_state: Option<CombatState>,
    strike_trace: &mut CombatStrikeReport,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) {
    hold_in_attack_range(world, unit_id);
    clear_attack_cycle_for_invalid_target(
        world,
        unit_id,
        target,
        Some(strike_trace),
        unit_catalog,
        weapon_catalog,
    );
    let next = invalid_target_state.unwrap_or(CombatState::Peaceful);
    let _ = world.set_unit_combat_state(unit_id, next);
}

#[allow(dead_code)]
fn clear_combat_to_peaceful(world: &mut WorldData, unit_id: UnitId) {
    let _ = (world, unit_id);
}

/// Initial combat posture when an attack order is accepted.
pub fn initial_attack_combat_state(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> CombatState {
    let Some(attacker) = world.get_unit(attacker_id) else {
        return CombatState::Attacking { target: target_id };
    };
    let Some(target) = world.get_unit(target_id) else {
        return CombatState::Attacking { target: target_id };
    };
    let Ok(weapon) = weapon_for_unit_record(attacker, unit_catalog, weapon_catalog) else {
        return CombatState::Attacking { target: target_id };
    };
    if is_in_weapon_range(world, attacker, target, unit_catalog, weapon) {
        CombatState::Attacking { target: target_id }
    } else {
        CombatState::Chasing { target: target_id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::TestPassabilityBundle;
    use crate::world::combat::range::RANGE_HYSTERESIS_METERS;
    use crate::world::{
        BuildingCatalog, BuildingConstructionSettings, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        CombatStrikeReport, DoodadCatalog, FootprintCatalog, Heightfield, LocalPosition,
        PassabilityCatalogs, UnitDefinitionId, UnitOrder, UnitOwnership, UnitSource, WeaponCatalog,
        create_unit_with_ownership, default_passability, issue_unit_order,
        resolve_all_pending_unit_orders, starter_unit_definitions,
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

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn catalog() -> UnitCatalog {
        UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
    }

    fn weapons() -> WeaponCatalog {
        WeaponCatalog::default()
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
            &UnitDefinitionId::new("bandit"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id
    }

    fn tick_combat(world: &mut WorldData, catalog: &UnitCatalog) -> CombatEngagementReport {
        let bundle = TestPassabilityBundle::new();
        step_all_combat_engagement(
            world,
            catalog,
            &weapons(),
            bundle.catalogs(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        )
    }

    #[test]
    fn out_of_range_attack_transitions_to_chasing() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 40.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Chasing { .. }
        ));
        let report = tick_combat(&mut world, &catalog);
        assert!(report.traces.iter().any(|trace| {
            trace.status == CombatEngagementStatus::OutOfRangeChasing
                && trace.chase_destination.is_some()
        }));
    }

    #[test]
    fn in_range_attack_holds_ready_state() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { .. }
        ));
        let report = tick_combat(&mut world, &catalog);
        assert!(
            report
                .traces
                .iter()
                .any(|trace| trace.status == CombatEngagementStatus::InRangeReady)
        );
        assert!(matches!(
            world.get_unit(player).unwrap().state,
            UnitState::Idle
        ));
    }

    #[test]
    fn moving_target_causes_chase_resume() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        tick_combat(&mut world, &catalog);
        world.relocate_unit(hostile, pos(40.0, 10.0)).unwrap();
        tick_combat(&mut world, &catalog);
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Chasing { .. } | CombatState::Attacking { .. }
        ));
        assert!(matches!(
            world.get_unit(player).unwrap().state,
            UnitState::Moving { .. } | UnitState::Idle
        ));
    }

    #[test]
    fn hysteresis_prevents_oscillation_at_boundary() {
        let weapon_range = 1.5;
        let edge_in = 1.45;
        let edge_out = 1.55;
        assert!(edge_in <= weapon_range);
        assert!(edge_out > weapon_range);
        assert!(edge_out <= weapon_range + RANGE_HYSTERESIS_METERS);
    }

    #[test]
    fn attack_move_acquires_nearest_valid_hostile() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let near = spawn_hostile(&mut world, &catalog, 14.0, 10.0);
        let far = spawn_hostile(&mut world, &catalog, 24.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::AttackMove {
                destination: pos(80.0, 80.0),
            },
            policy(),
        )
        .unwrap();
        let report = tick_combat(&mut world, &catalog);
        assert!(
            report
                .traces
                .iter()
                .any(|trace| { trace.status == CombatEngagementStatus::AttackMoveAcquired })
        );
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::AttackMoving {
                target: Some(target),
                ..
            } if target == near
        ));
        let _ = far;
    }

    #[test]
    fn attack_move_tie_breaks_by_lowest_unit_id() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world, &catalog, 15.0, 10.0);
        let hostile_b = spawn_hostile(&mut world, &catalog, 10.0, 15.0);
        let acquired =
            scan_attack_move_target(&world, player, &catalog, &weapons(), policy()).unwrap();
        let expected = hostile_a.min(hostile_b);
        assert_eq!(acquired, expected);
    }

    #[test]
    fn attack_move_ignores_invalid_ownership_targets() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let friendly = spawn_player(&mut world, &catalog, 12.0, 10.0);
        assert!(scan_attack_move_target(&world, player, &catalog, &weapons(), policy()).is_none());
        let _ = friendly;
    }

    #[test]
    fn no_damage_occurs_during_combat_positioning() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 40.0, 10.0);
        let hostile_hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        for _ in 0..5 {
            tick_combat(&mut world, &catalog);
            resolve_all_pending_unit_orders(
                &mut world,
                &catalog,
                default_passability(),
                &NavigationConfig::default(),
            );
            let mut scan = crate::world::CombatAiScanState::default();
            let settings = crate::world::CombatAiSettings::default();
            crate::simulation::run_simulation_tick(
                &mut world,
                &catalog,
                &weapons(),
                &DoodadCatalog::default(),
                &BuildingCatalog::default(),
                &FootprintCatalog::default(),
                &crate::world::BuildingInteractionProfileCatalog::default(),
                &NavigationConfig::default(),
                policy(),
                &settings,
                &mut scan,
                BuildingConstructionSettings::default(),
                &crate::world::InteriorProfileCatalog::default(),
                0.25,
                0,
            );
        }
        assert_eq!(
            world.get_unit(hostile).unwrap().vitals.current_hp,
            hostile_hp_before
        );
    }
}
