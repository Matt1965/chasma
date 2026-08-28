//! Reactive self-defense on attributed combat damage (ADR-062).
//!
//! Distinct from proactive combat AI scanning: retaliation issues a normal
//! [`UnitOrder::Attack`] when a unit without a valid combat target is hit by a
//! hostile it may legally attack.

use crate::world::unit::{
    UnitId, UnitOrder, apply_validated_attack_order, unit_can_execute_actions,
};
use crate::world::{
    AttackTargetingPolicy, DoodadCatalog, NavigationConfig, UnitCatalog, WeaponCatalog, WorldData,
    is_unit_alive, validate_reactive_retaliation_target,
};

use super::ai::unit_needs_auto_acquire_target;
use super::cycle_lifecycle::combat_engagement_target;
use crate::world::task::{TaskCancelReason, cancel_unit_task};

/// Apply attributed combat damage and attempt reactive self-defense.
///
/// Environmental or unattributed damage must use [`WorldData::damage_unit`] directly.
pub fn apply_attributed_combat_damage(
    world: &mut WorldData,
    victim_id: UnitId,
    attacker_id: UnitId,
    damage: u32,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
) -> Result<crate::world::unit::UnitVitals, crate::world::unit::UnitInsertError> {
    let hp_before = world
        .get_unit(victim_id)
        .map(|record| record.vitals.current_hp)
        .unwrap_or(0);
    #[cfg(feature = "dev")]
    super::runtime_trace::attributed_damage_called(attacker_id, victim_id, damage);
    let vitals = world.damage_unit(victim_id, damage)?;
    #[cfg(feature = "dev")]
    super::runtime_trace::victim_hp_before_after(victim_id, hp_before, vitals.current_hp);
    let retaliation_issued = try_reactive_combat_retaliation(
        world,
        victim_id,
        attacker_id,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        targeting_policy,
    );
    let _ = retaliation_issued;
    Ok(vitals)
}

/// Issue [`UnitOrder::Attack`] against `attacker_id` when retaliation rules allow.
///
/// Returns whether an attack order was issued.
pub fn try_reactive_combat_retaliation(
    world: &mut WorldData,
    victim_id: UnitId,
    attacker_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
) -> bool {
    let _ = (doodad_catalog, nav_config);
    if victim_id == attacker_id {
        #[cfg(feature = "dev")]
        super::runtime_trace::retaliation_result(
            victim_id,
            attacker_id,
            false,
            "skipped_self_target",
        );
        return false;
    }
    let Some(victim) = world.get_unit(victim_id).cloned() else {
        #[cfg(feature = "dev")]
        super::runtime_trace::retaliation_result(victim_id, attacker_id, false, "victim_missing");
        return false;
    };
    if !is_unit_alive(&victim) || !unit_can_execute_actions(world, victim_id) {
        #[cfg(feature = "dev")]
        super::runtime_trace::retaliation_result(
            victim_id,
            attacker_id,
            false,
            "victim_cannot_act",
        );
        return false;
    }
    if validate_reactive_retaliation_target(
        world,
        victim_id,
        attacker_id,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    )
    .is_err()
    {
        #[cfg(feature = "dev")]
        super::runtime_trace::retaliation_result(
            victim_id,
            attacker_id,
            false,
            "invalid_reactive_target",
        );
        return false;
    }
    if !unit_needs_auto_acquire_target(
        world,
        victim_id,
        &victim,
        weapon_catalog,
        unit_catalog,
        targeting_policy,
    ) {
        let kept = combat_engagement_target(&victim.combat_state) == Some(attacker_id);
        #[cfg(feature = "dev")]
        super::runtime_trace::retaliation_result(
            victim_id,
            attacker_id,
            kept,
            if kept {
                "already_fighting_attacker"
            } else {
                "already_has_valid_target"
            },
        );
        return kept;
    }
    let mut events = Vec::new();
    cancel_unit_task(world, victim_id, TaskCancelReason::PlayerOrder, &mut events);
    let issued = apply_validated_attack_order(
        world,
        unit_catalog,
        weapon_catalog,
        victim_id,
        attacker_id,
        Some(attacker_id),
    )
    .is_ok();
    #[cfg(feature = "dev")]
    super::runtime_trace::retaliation_result(
        victim_id,
        attacker_id,
        issued,
        if issued {
            "attack_order_issued"
        } else {
            "attack_order_failed"
        },
    );
    issued
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::run_simulation_tick;
    use crate::world::combat::test_support::phase6_authored_catalog;
    use crate::world::combat::{
        CombatAiScanState, CombatAiSettings, CombatStrikeEvent, CombatStrikeReport,
        step_combat_ai_acquisition,
    };
    use crate::world::task::{TaskPriority, TaskRecord, TaskState, TaskTarget, TaskType};
    use crate::world::unit::{CombatState, UnitState, resolve_all_pending_unit_orders};
    use crate::world::{
        Affiliation, AuthoredRelationshipCatalog, BuildingId, ChunkCoord, ChunkData, ChunkId,
        ChunkLayout, DoodadCatalog, FootprintCatalog, Heightfield, LocalPosition, NavigationConfig,
        PassabilityCatalogs, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource,
        WeaponCatalog, WorldPosition, create_unit_with_ownership, default_passability,
        issue_unit_order,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> crate::world::WorldData {
        let mut world = crate::world::WorldData::new(ChunkLayout {
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

    fn catalogs() -> (UnitCatalog, WeaponCatalog) {
        (UnitCatalog::default(), WeaponCatalog::default())
    }

    /// Starter weapons with wolf bite damage reduced so victims survive one strike.
    fn strike_test_weapons() -> WeaponCatalog {
        let base = WeaponCatalog::default();
        let definitions = base
            .definitions()
            .iter()
            .map(|weapon| {
                let mut weapon = weapon.clone();
                if weapon.id.as_str() == "weapon_wolf_bite" {
                    weapon.damage = 1.0;
                }
                weapon
            })
            .collect();
        WeaponCatalog::from_definitions(definitions).unwrap()
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    fn authored_relationships() -> AuthoredRelationshipCatalog {
        phase6_authored_catalog()
    }

    fn patch_player_faction(world: &mut crate::world::WorldData, unit_id: UnitId) {
        let mut record = world.remove_unit_by_id(unit_id).expect("unit exists");
        record.faction_id = crate::world::FactionId::new("player");
        let chunk = ChunkId::new(record.placement.position.chunk);
        world.insert_unit(chunk, record).unwrap();
    }

    fn spawn_player(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        x: f32,
        z: f32,
    ) -> UnitId {
        let id = create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        patch_player_faction(world, id);
        id
    }

    fn spawn_hostile(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        x: f32,
        z: f32,
    ) -> UnitId {
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

    fn spawn_wildlife_bandit(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        x: f32,
        z: f32,
    ) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("bandit"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id
    }

    fn spawn_player_bandit(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        x: f32,
        z: f32,
    ) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("bandit"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id
    }

    fn spawn_player_deer(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        x: f32,
        z: f32,
    ) -> UnitId {
        let id = create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("deer"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        patch_player_faction(world, id);
        id
    }

    fn spawn_hostile_at(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        affiliation: Affiliation,
        x: f32,
        z: f32,
    ) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(affiliation),
        )
        .unwrap()
        .id
    }

    fn issue_attack(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        weapons: &WeaponCatalog,
        attacker: UnitId,
        target: UnitId,
    ) {
        issue_unit_order(
            world,
            catalog,
            weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            attacker,
            UnitOrder::Attack { target },
            policy(),
        )
        .unwrap();
    }

    fn inflict_attributed_damage(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        weapons: &WeaponCatalog,
        attacker: UnitId,
        victim: UnitId,
        damage: u32,
    ) {
        apply_attributed_combat_damage(
            world,
            victim,
            attacker,
            damage,
            catalog,
            weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
        )
        .unwrap();
    }

    fn assign_working_task(
        world: &mut crate::world::WorldData,
        unit_id: UnitId,
    ) -> crate::world::TaskId {
        let task_id = world.task_store_mut().allocate_task_id();
        let building_id = BuildingId::new(1);
        let mut task = TaskRecord::new(
            task_id,
            TaskType::ConstructBuilding,
            TaskTarget::Building(building_id),
            TaskPriority::Normal,
            1,
        );
        task.state = TaskState::InProgress;
        task.assigned_unit_id = Some(unit_id);
        world.task_store_mut().insert_task(task).unwrap();
        world
            .task_store_mut()
            .assign_unit(task_id, unit_id)
            .unwrap();
        world.task_store_mut().get_mut(task_id).unwrap().state = TaskState::InProgress;
        world
            .set_unit_state(unit_id, UnitState::Working { task_id })
            .unwrap();
        task_id
    }

    fn step_tick(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        weapons: &WeaponCatalog,
        tick: u64,
        combat_ai_settings: &CombatAiSettings,
    ) -> crate::simulation::SimulationTickReport {
        let mut scan = CombatAiScanState::default();
        run_simulation_tick(
            world,
            catalog,
            weapons,
            &DoodadCatalog::default(),
            &crate::world::BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &authored_relationships(),
            combat_ai_settings,
            &mut scan,
            crate::world::BuildingConstructionSettings::default(),
            &crate::world::InteriorProfileCatalog::default(),
            None,
            &crate::world::ItemCatalog::default(),
            &crate::world::ItemCategoryCatalog::default(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::CorpseSettings::default(),
            1.0 / 30.0,
            tick,
            None,
        )
    }

    fn fast_combat_ai_settings() -> CombatAiSettings {
        CombatAiSettings {
            scan_interval_seconds: 0.0,
            ..CombatAiSettings::default()
        }
    }

    #[test]
    fn wildlife_retaliates_when_player_attacks() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_bandit(&mut world, &catalog, 10.0, 10.0);
        let wildlife = spawn_wildlife_bandit(&mut world, &catalog, 11.0, 10.0);
        inflict_attributed_damage(&mut world, &catalog, &weapons, player, wildlife, 1);
        assert!(matches!(
            world.get_unit(wildlife).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == player
        ));
    }

    #[test]
    fn idle_wildlife_near_player_does_not_proactively_attack() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_bandit(&mut world, &catalog, 10.0, 10.0);
        let wildlife = spawn_wildlife_bandit(&mut world, &catalog, 11.0, 10.0);
        let settings = fast_combat_ai_settings();
        let mut scan = CombatAiScanState::default();
        for tick in 1..=30 {
            step_combat_ai_acquisition(
                &mut world,
                &catalog,
                &weapons,
                &DoodadCatalog::default(),
                &NavigationConfig::default(),
                policy(),
                &AuthoredRelationshipCatalog::default(),
                &settings,
                &mut scan,
                1.0,
            );
            step_tick(&mut world, &catalog, &weapons, tick, &settings);
        }
        assert!(matches!(
            world.get_unit(wildlife).unwrap().combat_state,
            CombatState::Peaceful
        ));
    }

    #[test]
    fn wildlife_explicit_attack_on_player_is_mechanically_allowed() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_bandit(&mut world, &catalog, 10.0, 10.0);
        let wildlife = spawn_wildlife_bandit(&mut world, &catalog, 11.0, 10.0);
        assert!(
            issue_unit_order(
                &mut world,
                &catalog,
                &weapons,
                &DoodadCatalog::default(),
                &NavigationConfig::default(),
                wildlife,
                UnitOrder::Attack { target: player },
                policy(),
            )
            .is_ok()
        );
    }

    #[test]
    fn idle_player_retaliates_after_hostile_strike() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        inflict_attributed_damage(&mut world, &catalog, &weapons, hostile, player, 1);
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile
        ));
    }

    #[test]
    fn moving_player_retaliates_and_interrupts_movement() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::MoveTo {
                target: pos(80.0, 80.0),
            },
            policy(),
        )
        .unwrap();
        let _ = resolve_all_pending_unit_orders(
            &mut world,
            &catalog,
            default_passability(),
            &NavigationConfig::default(),
        );
        if !matches!(
            world.get_unit(player).unwrap().state,
            UnitState::Moving { .. }
        ) {
            world
                .set_unit_state(
                    player,
                    UnitState::Moving {
                        target: pos(80.0, 80.0),
                        path: crate::world::NavigationPath::from_surface_positions(vec![pos(
                            80.0, 80.0,
                        )]),
                        waypoint_index: 0,
                    },
                )
                .unwrap();
        }
        assert!(matches!(
            world.get_unit(player).unwrap().state,
            UnitState::Moving { .. }
        ));
        inflict_attributed_damage(&mut world, &catalog, &weapons, hostile, player, 1);
        assert!(!matches!(
            world.get_unit(player).unwrap().state,
            UnitState::Moving { .. }
        ));
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile
        ));
    }

    #[test]
    fn working_player_retaliates_and_cancels_task() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let task_id = assign_working_task(&mut world, player);
        inflict_attributed_damage(&mut world, &catalog, &weapons, hostile, player, 1);
        assert!(!matches!(
            world.get_unit(player).unwrap().state,
            UnitState::Working { .. }
        ));
        assert_eq!(
            world.task_store().get(task_id).unwrap().state,
            TaskState::Canceled
        );
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile
        ));
    }

    #[test]
    fn victim_fighting_hostile_a_keeps_target_when_hit_by_hostile_b() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile_at(&mut world, &catalog, Affiliation::Hostile, 11.0, 10.0);
        let hostile_b = spawn_hostile_at(&mut world, &catalog, Affiliation::Hostile, 10.0, 11.0);
        issue_attack(&mut world, &catalog, &weapons, player, hostile_a);
        inflict_attributed_damage(&mut world, &catalog, &weapons, hostile_b, player, 1);
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile_a
        ));
    }

    #[test]
    fn victim_with_invalid_target_retaliates_against_new_attacker() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world, &catalog, 50.0, 50.0);
        let hostile_b = spawn_hostile_at(&mut world, &catalog, Affiliation::Hostile, 11.0, 10.0);
        world.damage_unit(hostile_a, 999).unwrap();
        world
            .set_unit_combat_state(player, CombatState::Attacking { target: hostile_a })
            .unwrap();
        inflict_attributed_damage(&mut world, &catalog, &weapons, hostile_b, player, 1);
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile_b
        ));
    }

    #[test]
    fn friendly_attacker_does_not_trigger_retaliation() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player_a = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let player_b = spawn_player(&mut world, &catalog, 11.0, 10.0);
        apply_attributed_combat_damage(
            &mut world,
            player_a,
            player_b,
            1,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
        )
        .unwrap();
        assert!(matches!(
            world.get_unit(player_a).unwrap().combat_state,
            CombatState::Peaceful
        ));
    }

    #[test]
    fn unattributed_damage_does_not_trigger_retaliation() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.damage_unit(player, 1).unwrap();
        let _ = weapons;
        let _ = hostile;
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Peaceful
        ));
    }

    #[test]
    fn dead_victim_does_not_retaliate() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.damage_unit(player, 999).unwrap();
        try_reactive_combat_retaliation(
            &mut world,
            player,
            hostile,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
        );
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Peaceful
        ));
    }

    #[test]
    fn strike_resolution_triggers_retaliation() {
        use crate::world::combat::{
            CombatStrikeEvent, CombatStrikeReport, step_all_combat_engagement,
            step_all_combat_strikes,
        };

        let (catalog, _) = catalogs();
        let weapons = strike_test_weapons();
        let mut world = flat_world();
        let player = spawn_player_deer(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_attack(&mut world, &catalog, &weapons, hostile, player);
        let mut damaged = false;
        for _ in 0..120 {
            let mut strike_report = CombatStrikeReport::default();
            let mut projectile = crate::world::ProjectileReport::default();
            step_all_combat_engagement(
                &mut world,
                &catalog,
                &weapons,
                default_passability(),
                &NavigationConfig::default(),
                policy(),
                &AuthoredRelationshipCatalog::default(),
                &mut strike_report,
            );
            strike_report = step_all_combat_strikes(
                &mut world,
                &catalog,
                &weapons,
                &DoodadCatalog::default(),
                &NavigationConfig::default(),
                policy(),
                1.0 / 30.0,
                &mut projectile,
            );
            if strike_report.traces.iter().any(|trace| {
                trace.target_id == player
                    && matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. })
            }) {
                damaged = true;
                break;
            }
        }
        assert!(damaged, "hostile should strike player");
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile
        ));
    }

    #[test]
    fn hostile_ai_damages_idle_player_and_player_retaliates_over_ticks() {
        let (catalog, _) = catalogs();
        let weapons = strike_test_weapons();
        let mut world = flat_world();
        let player = spawn_player_deer(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let settings = fast_combat_ai_settings();
        let hp_before = world.get_unit(player).unwrap().vitals.current_hp;
        let mut damaged = false;
        for tick in 1..=120 {
            step_tick(&mut world, &catalog, &weapons, tick, &settings);
            let Some(player_unit) = world.get_unit(player) else {
                break;
            };
            if player_unit.vitals.current_hp < hp_before {
                damaged = true;
                assert!(matches!(
                    player_unit.combat_state,
                    CombatState::Attacking { target } | CombatState::Chasing { target }
                        if target == hostile
                ));
                break;
            }
        }
        assert!(
            damaged,
            "hostile should damage player over simulation ticks"
        );
    }

    #[test]
    fn end_to_end_hostile_acquire_retaliate_and_death() {
        let (catalog, _) = catalogs();
        let weapons = strike_test_weapons();
        let mut world = flat_world();
        let player = spawn_player_deer(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 12).unwrap();
        let settings = fast_combat_ai_settings();
        let mut scan = CombatAiScanState::default();
        step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &AuthoredRelationshipCatalog::default(),
            &settings,
            &mut scan,
            1.0,
        );
        inflict_attributed_damage(&mut world, &catalog, &weapons, hostile, player, 1);
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile
        ));
        for tick in 1..=240 {
            let _ = step_tick(&mut world, &catalog, &weapons, tick, &settings);
            if world.get_unit(hostile).is_none() {
                break;
            }
        }
        assert!(world.get_unit(hostile).is_none());
    }

    fn step_combat_only(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        weapons: &WeaponCatalog,
    ) -> (
        crate::world::CombatEngagementReport,
        crate::world::CombatStrikeReport,
    ) {
        use crate::world::combat::{step_all_combat_engagement, step_all_combat_strikes};
        let mut strike_report = CombatStrikeReport::default();
        let mut projectile = crate::world::ProjectileReport::default();
        let engagement = step_all_combat_engagement(
            world,
            catalog,
            weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &AuthoredRelationshipCatalog::default(),
            &mut strike_report,
        );
        strike_report = step_all_combat_strikes(
            world,
            catalog,
            weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            1.0 / 30.0,
            &mut projectile,
        );
        (engagement, strike_report)
    }

    #[test]
    fn wildlife_reactive_combat_survives_engagement_and_strikes_player() {
        use crate::world::CombatEngagementStatus;

        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_bandit(&mut world, &catalog, 10.0, 10.0);
        let wildlife = spawn_wildlife_bandit(&mut world, &catalog, 10.8, 10.0);
        let player_hp_before = world.get_unit(player).unwrap().vitals.current_hp;
        issue_attack(&mut world, &catalog, &weapons, player, wildlife);

        let mut wildlife_damaged = false;
        let mut retaliating = false;
        let mut engagement_valid_after_retaliation = false;
        let mut wildlife_windup = false;
        let mut player_damaged_by_wildlife = false;

        for _ in 0..240 {
            let (engagement, strike_report) = step_combat_only(&mut world, &catalog, &weapons);
            let wildlife_hp = world.get_unit(wildlife).unwrap().vitals.current_hp;
            if wildlife_hp < 8 {
                wildlife_damaged = true;
            }
            if world.get_unit(wildlife).unwrap().reactive_combat_target == Some(player) {
                retaliating = true;
            }
            if retaliating {
                for trace in &engagement.traces {
                    if trace.unit_id == wildlife {
                        assert_ne!(trace.status, CombatEngagementStatus::TargetInvalid);
                        engagement_valid_after_retaliation = true;
                    }
                }
            }
            if strike_report.traces.iter().any(|trace| {
                trace.attacker_id == wildlife
                    && trace.target_id == player
                    && matches!(trace.event, CombatStrikeEvent::AttackWindupStarted)
            }) {
                wildlife_windup = true;
            }
            if strike_report.traces.iter().any(|trace| {
                trace.attacker_id == wildlife
                    && trace.target_id == player
                    && matches!(trace.event, CombatStrikeEvent::AttackStrikeApplied { .. })
            }) {
                player_damaged_by_wildlife = true;
                break;
            }
        }

        assert!(wildlife_damaged, "player should damage wildlife first");
        assert!(retaliating, "wildlife should gain reactive authorization");
        assert_eq!(
            world.get_unit(wildlife).unwrap().reactive_combat_target,
            Some(player)
        );
        assert!(
            engagement_valid_after_retaliation,
            "engagement must not invalidate reactive target on next tick"
        );
        assert!(wildlife_windup, "wildlife should start attack windup");
        assert!(
            player_damaged_by_wildlife,
            "wildlife should strike player after reactive authorization persists"
        );
        assert!(
            world.get_unit(player).unwrap().vitals.current_hp < player_hp_before,
            "player hp should decrease"
        );
    }

    #[test]
    fn reactive_authorization_survives_repeated_engagement_ticks() {
        use crate::world::CombatEngagementStatus;

        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_bandit(&mut world, &catalog, 10.0, 10.0);
        let wildlife = spawn_wildlife_bandit(&mut world, &catalog, 10.8, 10.0);
        inflict_attributed_damage(&mut world, &catalog, &weapons, player, wildlife, 1);
        assert_eq!(
            world.get_unit(wildlife).unwrap().reactive_combat_target,
            Some(player)
        );
        for _ in 0..10 {
            let (engagement, _) = step_combat_only(&mut world, &catalog, &weapons);
            for trace in &engagement.traces {
                if trace.unit_id == wildlife {
                    assert_ne!(trace.status, CombatEngagementStatus::TargetInvalid);
                }
            }
            assert_eq!(
                world.get_unit(wildlife).unwrap().reactive_combat_target,
                Some(player)
            );
        }
    }

    #[test]
    fn move_to_clears_reactive_authorization() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_bandit(&mut world, &catalog, 10.0, 10.0);
        let wildlife = spawn_wildlife_bandit(&mut world, &catalog, 10.8, 10.0);
        inflict_attributed_damage(&mut world, &catalog, &weapons, player, wildlife, 1);
        assert_eq!(
            world.get_unit(wildlife).unwrap().reactive_combat_target,
            Some(player)
        );
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            wildlife,
            UnitOrder::MoveTo {
                target: pos(40.0, 40.0),
            },
            policy(),
        )
        .unwrap();
        assert_eq!(
            world.get_unit(wildlife).unwrap().reactive_combat_target,
            None
        );
        assert!(matches!(
            world.get_unit(wildlife).unwrap().combat_state,
            CombatState::Peaceful
        ));
    }

    #[test]
    fn idle_clears_reactive_authorization() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_bandit(&mut world, &catalog, 10.0, 10.0);
        let wildlife = spawn_wildlife_bandit(&mut world, &catalog, 10.8, 10.0);
        inflict_attributed_damage(&mut world, &catalog, &weapons, player, wildlife, 1);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            wildlife,
            UnitOrder::Idle,
            policy(),
        )
        .unwrap();
        assert_eq!(
            world.get_unit(wildlife).unwrap().reactive_combat_target,
            None
        );
    }

    #[test]
    fn explicit_attack_on_different_target_clears_reactive_authorization() {
        let (catalog, weapons) = catalogs();
        let mut world = flat_world();
        let player = spawn_player_deer(&mut world, &catalog, 10.0, 10.0);
        let hostile_a = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hostile_b = spawn_hostile_at(&mut world, &catalog, Affiliation::Hostile, 12.0, 10.0);
        inflict_attributed_damage(&mut world, &catalog, &weapons, hostile_a, player, 1);
        assert_eq!(
            world.get_unit(player).unwrap().reactive_combat_target,
            Some(hostile_a)
        );
        issue_attack(&mut world, &catalog, &weapons, player, hostile_b);
        assert_eq!(world.get_unit(player).unwrap().reactive_combat_target, None);
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == hostile_b
        ));
    }
}
