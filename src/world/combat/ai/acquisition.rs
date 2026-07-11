//! Combat auto-acquisition — scan, prioritize, issue attack orders (ADR-062 C9).

use bevy::prelude::*;

use crate::world::navigation::xz_distance;
use crate::world::ownership::is_player_controllable;
use crate::world::unit::{CombatState, UnitId, UnitOrder, UnitState, unit_can_execute_actions};
use crate::world::{
    AttackTargetingPolicy, DoodadCatalog, NavigationConfig, UnitCatalog, WeaponCatalog, WorldData,
    is_unit_alive, is_valid_attack_target, issue_unit_order, validate_attack_target,
    weapon_for_unit_record,
};

use super::settings::CombatAiSettings;

/// Round-robin scan cursor persisted across simulation ticks.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct CombatAiScanState {
    pub cursor: usize,
    pub seconds_since_scan: f32,
}

/// Trace outcome for one AI scan attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatAiTraceOutcome {
    AiTargetAcquired,
    AiScanNoTarget,
    AiScanSkippedBudget,
    AiScanSkippedInterval,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CombatAiTrace {
    pub unit_id: UnitId,
    pub outcome: CombatAiTraceOutcome,
    pub target: Option<UnitId>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatAiReport {
    pub traces: Vec<CombatAiTrace>,
}

impl CombatAiReport {
    pub fn push(&mut self, trace: CombatAiTrace) {
        self.traces.push(trace);
    }
}

/// Scan eligible units and issue [`UnitOrder::Attack`] through the order API.
pub fn step_combat_ai_acquisition(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    settings: &CombatAiSettings,
    scan_state: &mut CombatAiScanState,
    delta_seconds: f32,
) -> CombatAiReport {
    let mut report = CombatAiReport::default();
    if !settings.enabled {
        return report;
    }

    scan_state.seconds_since_scan += delta_seconds;
    if scan_state.seconds_since_scan + f32::EPSILON < settings.scan_interval_seconds {
        return report;
    }
    scan_state.seconds_since_scan = 0.0;

    let unit_ids = world.sorted_unit_ids();
    let len = unit_ids.len();
    if len == 0 {
        return report;
    }

    let budget = settings.max_units_scanned_per_tick.min(len);
    for offset in 0..budget {
        let index = (scan_state.cursor + offset) % len;
        let unit_id = unit_ids[index];
        let trace = scan_unit_for_acquisition(
            world,
            unit_catalog,
            weapon_catalog,
            doodad_catalog,
            nav_config,
            targeting_policy,
            settings,
            unit_id,
        );
        if let Some(trace) = trace {
            report.push(trace);
        }
    }

    scan_state.cursor = (scan_state.cursor + budget) % len;
    report
}

fn scan_unit_for_acquisition(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    settings: &CombatAiSettings,
    unit_id: UnitId,
) -> Option<CombatAiTrace> {
    let record = world.get_unit(unit_id)?;
    if !unit_can_execute_actions(world, unit_id) {
        return None;
    }
    if !unit_eligible_for_auto_acquire(record, settings) {
        return None;
    }
    if !unit_needs_auto_acquire_target(
        world,
        unit_id,
        record,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    ) {
        return None;
    }
    if weapon_for_unit_record(record, unit_catalog, weapon_catalog).is_err() {
        return None;
    }

    let Some(target) = find_auto_acquire_target(
        world,
        unit_id,
        unit_catalog,
        weapon_catalog,
        targeting_policy,
        settings.scan_radius_meters,
    ) else {
        return Some(CombatAiTrace {
            unit_id,
            outcome: CombatAiTraceOutcome::AiScanNoTarget,
            target: None,
        });
    };

    issue_unit_order(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        unit_id,
        UnitOrder::Attack { target },
        targeting_policy,
    )
    .ok()?;

    Some(CombatAiTrace {
        unit_id,
        outcome: CombatAiTraceOutcome::AiTargetAcquired,
        target: Some(target),
    })
}

pub fn unit_eligible_for_auto_acquire(
    record: &crate::world::UnitRecord,
    settings: &CombatAiSettings,
) -> bool {
    if !is_unit_alive(record) {
        return false;
    }
    if is_player_controllable(record) && !settings.player_units_auto_acquire {
        return false;
    }
    match &record.state {
        UnitState::Dead => false,
        UnitState::Idle => true,
        UnitState::Moving { .. } => matches!(record.combat_state, CombatState::AttackMoving { .. }),
    }
}

pub fn unit_needs_auto_acquire_target(
    world: &WorldData,
    unit_id: UnitId,
    record: &crate::world::UnitRecord,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> bool {
    match &record.combat_state {
        CombatState::Peaceful | CombatState::Alert | CombatState::Engaged => true,
        CombatState::AttackMoving { target, .. } => target.is_none_or(|target_id| {
            !is_valid_attack_target(
                world,
                unit_id,
                target_id,
                weapon_catalog,
                unit_catalog,
                policy,
            )
        }),
        CombatState::Attacking { target } | CombatState::Chasing { target } => {
            !is_valid_attack_target(
                world,
                unit_id,
                *target,
                weapon_catalog,
                unit_catalog,
                policy,
            )
        }
    }
}

/// Nearest valid target within radius; closest distance, then lowest [`UnitId`].
pub fn find_auto_acquire_target(
    world: &WorldData,
    attacker_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    targeting_policy: AttackTargetingPolicy,
    scan_radius_meters: f32,
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
        if distance > scan_radius_meters {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WeaponCatalog, WorldPosition,
        create_unit_with_ownership, starter_unit_definitions, starter_weapon_definitions,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
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

    fn test_catalogs() -> (UnitCatalog, WeaponCatalog) {
        (
            UnitCatalog::from_definitions(starter_unit_definitions()).unwrap(),
            WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap(),
        )
    }

    fn spawn_with_affiliation(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        affiliation: Affiliation,
        position: WorldPosition,
    ) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("bandit"),
            position,
            UnitSource::Authored,
            UnitOwnership::with_affiliation(affiliation),
        )
        .unwrap()
        .id
    }

    #[test]
    fn hostile_unit_acquires_player_unit() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(4.0, 0.0));
        let mut scan = CombatAiScanState::default();
        let settings = CombatAiSettings::default();
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &settings,
            &mut scan,
            1.0,
        );
        assert!(report.traces.iter().any(
            |t| t.outcome == CombatAiTraceOutcome::AiTargetAcquired && t.target == Some(player)
        ));
        assert!(matches!(
            world.get_unit(hostile).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == player
        ));
    }

    #[test]
    fn player_unit_does_not_auto_acquire_by_default() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let _hostile =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(0.0, 0.0));
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(4.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let mut scan = CombatAiScanState::default();
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &CombatAiSettings::default(),
            &mut scan,
            1.0,
        );
        assert!(!report.traces.iter().any(|t| t.unit_id == player));
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Peaceful
        ));
    }

    #[test]
    fn player_auto_acquire_setting_enables_player_acquisition() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let hostile =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(0.0, 0.0));
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(4.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let mut scan = CombatAiScanState::default();
        let settings = CombatAiSettings {
            player_units_auto_acquire: true,
            ..Default::default()
        };
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &settings,
            &mut scan,
            1.0,
        );
        assert!(
            report
                .traces
                .iter()
                .any(|t| t.unit_id == player && t.target == Some(hostile))
        );
    }

    #[test]
    fn neutral_unit_does_not_acquire_by_default() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let _player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        let neutral =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Neutral, pos(4.0, 0.0));
        let mut scan = CombatAiScanState::default();
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &CombatAiSettings::default(),
            &mut scan,
            1.0,
        );
        assert!(!report.traces.iter().any(|t| {
            t.unit_id == neutral && t.outcome == CombatAiTraceOutcome::AiTargetAcquired
        }));
        assert_eq!(
            world.get_unit(neutral).unwrap().combat_state,
            CombatState::Peaceful
        );
    }

    #[test]
    fn dead_unit_does_not_acquire() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let _player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        let hostile =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(4.0, 0.0));
        world
            .set_unit_state(hostile, UnitState::Dead)
            .expect("set dead");
        let mut scan = CombatAiScanState::default();
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &CombatAiSettings::default(),
            &mut scan,
            1.0,
        );
        assert!(!report.traces.iter().any(|t| t.unit_id == hostile));
    }

    #[test]
    fn closest_target_chosen() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let hostile =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(0.0, 0.0));
        let near = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(3.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let _far = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(10.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        let target = find_auto_acquire_target(
            &world,
            hostile,
            &catalog,
            &weapons,
            AttackTargetingPolicy::default(),
            24.0,
        )
        .unwrap();
        assert_eq!(target, near);
    }

    #[test]
    fn lowest_unit_id_tie_break() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let hostile =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(0.0, 0.0));
        let a = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(5.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let b = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let target = find_auto_acquire_target(
            &world,
            hostile,
            &catalog,
            &weapons,
            AttackTargetingPolicy::default(),
            24.0,
        )
        .unwrap();
        assert_eq!(target, a.min(b));
    }

    #[test]
    fn scan_budget_limits_units_per_tick() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let _player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        for i in 0..4 {
            spawn_with_affiliation(
                &mut world,
                &catalog,
                Affiliation::Hostile,
                pos(4.0 + i as f32, 0.0),
            );
        }
        let settings = CombatAiSettings {
            max_units_scanned_per_tick: 2,
            ..Default::default()
        };
        let mut scan = CombatAiScanState::default();
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &settings,
            &mut scan,
            1.0,
        );
        assert!(report.traces.len() <= 2);
        assert_eq!(scan.cursor, 2);
    }

    #[test]
    fn scan_interval_prevents_every_frame_scans() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let _player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(4.0, 0.0));
        let settings = CombatAiSettings {
            scan_interval_seconds: 1.0,
            ..Default::default()
        };
        let mut scan = CombatAiScanState::default();
        let first = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &settings,
            &mut scan,
            0.1,
        );
        assert!(first.traces.is_empty());
        let second = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &settings,
            &mut scan,
            1.0,
        );
        assert!(!second.traces.is_empty());
    }

    #[test]
    fn ai_issues_attack_order_through_order_api() {
        let (catalog, weapons) = test_catalogs();
        let mut world = flat_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile =
            spawn_with_affiliation(&mut world, &catalog, Affiliation::Hostile, pos(4.0, 0.0));
        let before = world.get_unit(hostile).unwrap().combat_state.clone();
        assert_eq!(before, CombatState::Peaceful);
        let mut scan = CombatAiScanState::default();
        step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &CombatAiSettings::default(),
            &mut scan,
            1.0,
        );
        assert_ne!(world.get_unit(hostile).unwrap().combat_state, before);
        assert!(matches!(
            world.get_unit(hostile).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == player
        ));
    }

    #[test]
    fn unit_with_missing_weapon_does_not_acquire() {
        use crate::world::{UnitDefinition, UnitRenderKey, WeaponDefinitionId};

        let mut unit_defs = starter_unit_definitions();
        unit_defs.push(UnitDefinition::new(
            UnitDefinitionId::new("unarmed_hostile"),
            "Unarmed",
            "Test",
            1,
            5,
            5,
            4,
            4,
            4,
            4,
            4,
            4,
            20.0,
            "Common",
            3.0,
            0.5,
            30.0,
            WeaponDefinitionId::new("missing_weapon"),
            true,
            UnitRenderKey::reserved("bandit"),
        ));
        let catalog = UnitCatalog::from_definitions(unit_defs).unwrap();
        let weapons = WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap();
        let mut world = flat_world();
        let _player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        let hostile = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("unarmed_hostile"),
            pos(4.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;
        let mut scan = CombatAiScanState::default();
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &CombatAiSettings::default(),
            &mut scan,
            1.0,
        );
        assert!(
            !report
                .traces
                .iter()
                .any(|t| t.outcome == CombatAiTraceOutcome::AiTargetAcquired)
        );
        assert_eq!(
            world.get_unit(hostile).unwrap().combat_state,
            CombatState::Peaceful
        );
    }
}
