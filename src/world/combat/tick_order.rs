//! Combat simulation tick ordering regression tests (REVIEW-A4).

#[cfg(test)]
mod tests {
    use crate::simulation::{SIMULATION_TICK_SECONDS, SimulationControlState};
    use crate::world::combat::{
        CombatAiScanState, CombatAiSettings, CombatStrikeEvent, step_all_combat_engagement,
        step_all_combat_strikes, step_combat_ai_acquisition,
    };
    use crate::world::projectile::{ProjectileEvent, step_all_projectiles};
    use crate::world::unit::{
        AttackCycle, AttackPhase, RemovalReason, UnitDeathEvent, queue_unit_removal,
        step_unit_death_pipeline,
    };
    use crate::world::{
        AttackTargetingPolicy, BuildingCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        CombatEngagementStatus, CombatState, DamageType, DoodadCatalog, FootprintCatalog,
        Heightfield, HitMode, LocalPosition, NavigationConfig, PassabilityCatalogs,
        ProjectileReport, TargetFilter, UnitCatalog, UnitDefinitionId, UnitId, UnitOrder,
        UnitOwnership, UnitSource, WeaponCatalog, WeaponDefinition, WeaponDefinitionId, WorldData,
        WorldPosition, create_unit_with_ownership, default_passability, issue_unit_order,
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

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn catalog() -> UnitCatalog {
        UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    fn weapons() -> WeaponCatalog {
        WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
    }

    fn projectile_weapon(speed_mps: f32) -> WeaponCatalog {
        let mut defs = starter_weapon_definitions();
        defs.push(WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_test_bow"),
            "Test Bow",
            "Test",
            5.0,
            DamageType::Piercing,
            15.0,
            1.0,
            0.1,
            0.1,
            HitMode::Projectile,
            Some("arrow".to_string()),
            speed_mps,
            "attack_bow",
            vec![TargetFilter::Enemies],
            None,
            true,
        ));
        WeaponCatalog::from_definitions(defs).unwrap()
    }

    fn catalog_with_projectile_weapon(weapons: &WeaponCatalog) -> UnitCatalog {
        let base = catalog();
        let mut wolf = base.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        wolf.default_weapon_id = WeaponDefinitionId::new("weapon_test_bow");
        UnitCatalog::from_definitions(vec![
            wolf,
            base.get(&UnitDefinitionId::new("bandit")).unwrap().clone(),
        ])
        .unwrap()
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

    fn reassign_position(world: &mut WorldData, unit_id: UnitId, x: f32, z: f32) {
        let mut record = world.remove_unit_by_id(unit_id).expect("unit");
        record.placement.position = pos(x, z);
        world
            .insert_unit(ChunkId::new(record.placement.position.chunk), record)
            .unwrap();
    }

    fn step_tick(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        weapon_catalog: &WeaponCatalog,
    ) -> crate::simulation::SimulationTickReport {
        let mut scan = CombatAiScanState::default();
        let settings = CombatAiSettings::default();
        crate::simulation::run_simulation_tick(
            world,
            catalog,
            weapon_catalog,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &settings,
            &mut scan,
            crate::world::BuildingConstructionSettings::default(),
            &crate::world::InteriorProfileCatalog::default(),
            &crate::world::ItemCatalog::default(),
            &crate::world::ItemCategoryCatalog::default(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::CorpseSettings::default(),
            0.2,
            1,
            None,
        )
    }

    #[test]
    fn engagement_clears_windup_before_strike_when_out_of_range() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle {
                    target: hostile,
                    phase: AttackPhase::Windup,
                    phase_remaining_seconds: 0.05,
                    struck_this_cycle: false,
                }),
            )
            .unwrap();
        reassign_position(&mut world, hostile, 50.0, 10.0);
        let report = step_tick(&mut world, &catalog, &weapons);
        assert!(
            !report.combat_strike.traces.iter().any(|trace| {
                matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. })
            })
        );
        assert!(world.get_unit(player).unwrap().attack_cycle.is_none());
        assert!(matches!(
            report.combat.traces.first().map(|trace| trace.status),
            Some(CombatEngagementStatus::OutOfRangeChasing)
        ));
    }

    #[test]
    fn zero_hp_attacker_cannot_advance_windup() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.damage_unit(player, 999).unwrap();
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle {
                    target: hostile,
                    phase: AttackPhase::Windup,
                    phase_remaining_seconds: 0.05,
                    struck_this_cycle: false,
                }),
            )
            .unwrap();
        let report = step_tick(&mut world, &catalog, &weapons);
        assert!(report.combat_strike.traces.is_empty());
        if let Some(record) = world.get_unit(player) {
            assert!(record.attack_cycle.is_none());
        }
    }

    #[test]
    fn queued_for_removal_unit_cannot_receive_orders() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        assert!(queue_unit_removal(
            &mut world,
            unit,
            RemovalReason::Cleanup,
            None,
            1
        ));
        let err = issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            unit,
            UnitOrder::Idle,
            policy(),
        );
        assert!(matches!(
            err,
            Err(crate::world::UnitOrderError::UnitNotFound)
        ));
    }

    #[test]
    fn dead_attacker_cannot_launch_projectile() {
        let weapons = projectile_weapon(20.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle {
                    target: hostile,
                    phase: AttackPhase::Windup,
                    phase_remaining_seconds: 0.01,
                    struck_this_cycle: false,
                }),
            )
            .unwrap();
        world.damage_unit(player, 999).unwrap();
        let mut projectile = ProjectileReport::default();
        step_all_combat_strikes(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            0.2,
            &mut projectile,
        );
        assert!(!projectile.has_event(&ProjectileEvent::Spawned));
    }

    #[test]
    fn second_same_tick_striker_misses_after_target_killed() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let first = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let second = spawn_player(&mut world, &catalog, 10.0, 12.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 4).unwrap();
        for attacker in [first, second] {
            world
                .set_unit_combat_state(attacker, CombatState::Attacking { target: hostile })
                .unwrap();
            world
                .set_unit_attack_cycle(
                    attacker,
                    Some(AttackCycle {
                        target: hostile,
                        phase: AttackPhase::Windup,
                        phase_remaining_seconds: 0.01,
                        struck_this_cycle: false,
                    }),
                )
                .unwrap();
        }
        let report = step_tick(&mut world, &catalog, &weapons);
        let applied: Vec<_> = report
            .combat_strike
            .traces
            .iter()
            .filter(|trace| matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. }))
            .collect();
        assert_eq!(applied.len(), 1);
        assert!(world.get_unit(hostile).is_none());
    }

    #[test]
    fn death_queued_exactly_once_per_unit() {
        let catalog = catalog();
        let mut world = flat_world();
        let hostile = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        world.damage_unit(hostile, 999).unwrap();
        let report = step_unit_death_pipeline(
            &mut world,
            &catalog,
            None,
            &crate::world::CorpseSettings::default(),
            7,
        );
        let queued = report
            .traces
            .iter()
            .filter(|trace| matches!(trace.event, UnitDeathEvent::UnitRemovalQueued { .. }))
            .count();
        assert_eq!(queued, 1);
    }

    #[test]
    fn projectile_impact_death_uses_shared_death_pipeline() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 5).unwrap();
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle {
                    target: hostile,
                    phase: AttackPhase::Windup,
                    phase_remaining_seconds: 0.01,
                    struck_this_cycle: false,
                }),
            )
            .unwrap();
        let report = step_tick(&mut world, &catalog, &weapons);
        assert!(report.projectile.has_event(&ProjectileEvent::Spawned));
        let report = step_tick(&mut world, &catalog, &weapons);
        assert!(
            report
                .death
                .traces
                .iter()
                .any(|trace| { matches!(trace.event, UnitDeathEvent::UnitDied { .. }) })
        );
        assert!(world.get_unit(hostile).is_none());
    }

    #[test]
    fn newly_spawned_projectile_skips_movement_same_tick() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 20.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        let report = step_tick(&mut world, &catalog, &weapons);
        assert!(report.projectile.has_event(&ProjectileEvent::Spawned));
        let projectile = world.projectiles().next().unwrap().1;
        let spawn_position = projectile.position;
        assert_eq!(spawn_position.local.0.x, 10.0);
    }

    #[test]
    fn ai_does_not_acquire_for_queued_unit() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let unit = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        let nearby = spawn_player(&mut world, &catalog, 12.0, 10.0);
        let _ = nearby;
        assert!(queue_unit_removal(
            &mut world,
            unit,
            RemovalReason::Cleanup,
            None,
            1
        ));
        let mut scan = CombatAiScanState::default();
        let mut settings = CombatAiSettings::default();
        settings.scan_interval_seconds = 0.0;
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &settings,
            &mut scan,
            0.2,
        );
        assert!(!report.traces.iter().any(|trace| trace.unit_id == unit));
    }

    #[test]
    fn ai_runs_after_death_cleanup_in_tick_report() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 4).unwrap();
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle {
                    target: hostile,
                    phase: AttackPhase::Windup,
                    phase_remaining_seconds: 0.01,
                    struck_this_cycle: false,
                }),
            )
            .unwrap();
        let report = step_tick(&mut world, &catalog, &weapons);
        assert!(world.get_unit(hostile).is_none());
        assert!(report.death.removed_unit_ids.contains(&hostile));
        let mut scan = CombatAiScanState::default();
        let mut settings = CombatAiSettings::default();
        settings.scan_interval_seconds = 0.0;
        let ai_report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &settings,
            &mut scan,
            0.2,
        );
        assert!(
            !ai_report
                .traces
                .iter()
                .any(|trace| trace.target == Some(hostile))
        );
    }

    #[test]
    fn death_trace_events_are_ordered_within_pipeline() {
        let catalog = catalog();
        let mut world = flat_world();
        let hostile = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        world.damage_unit(hostile, 999).unwrap();
        let report = step_unit_death_pipeline(
            &mut world,
            &catalog,
            None,
            &crate::world::CorpseSettings::default(),
            7,
        );
        let events: Vec<_> = report
            .traces
            .iter()
            .filter(|trace| trace.unit_id == hostile)
            .map(|trace| &trace.event)
            .collect();
        assert!(matches!(events[0], UnitDeathEvent::UnitDied { .. }));
        assert!(matches!(
            events[1],
            UnitDeathEvent::UnitRemovalQueued { .. }
        ));
        assert!(matches!(
            events.last(),
            Some(UnitDeathEvent::UnitRemoved { .. })
        ));
    }

    #[test]
    fn engagement_runs_before_strike_in_isolated_pipeline() {
        let catalog = catalog();
        let weapons = weapons();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 50.0, 10.0);
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
            .unwrap();
        world
            .set_unit_attack_cycle(
                player,
                Some(AttackCycle {
                    target: hostile,
                    phase: AttackPhase::Windup,
                    phase_remaining_seconds: 0.01,
                    struck_this_cycle: false,
                }),
            )
            .unwrap();
        let mut strike_report = crate::world::CombatStrikeReport::default();
        let engagement = step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut strike_report,
        );
        assert!(matches!(
            engagement.traces.first().map(|trace| trace.status),
            Some(CombatEngagementStatus::OutOfRangeChasing)
        ));
        let mut projectile = ProjectileReport::default();
        let strikes = step_all_combat_strikes(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            0.2,
            &mut projectile,
        );
        assert!(
            !strikes.traces.iter().any(|trace| {
                matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. })
            })
        );
    }

    #[test]
    fn pause_step_runs_single_ordered_tick() {
        let mut control = SimulationControlState {
            paused: true,
            step_once: true,
            ..Default::default()
        };
        assert!(control.begin_tick());
        control.complete_tick();
        assert_eq!(control.current_tick, 1);
        assert!(!control.begin_tick());
    }

    #[test]
    fn spawned_projectile_advances_next_tick() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 20.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        let _ = step_tick(&mut world, &catalog, &weapons);
        let before = world.projectiles().next().unwrap().1.position.local.0.x;
        let _ = step_all_projectiles(&mut world, SIMULATION_TICK_SECONDS, &[]);
        let after = world.projectiles().next().unwrap().1.position.local.0.x;
        assert!(after > before);
    }
}
