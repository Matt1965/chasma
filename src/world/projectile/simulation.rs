//! Authoritative projectile movement and impact resolution (ADR-060 C7).

use crate::world::WorldData;
use crate::world::combat::{ProjectileImpactRejection, validate_projectile_impact_target};
use crate::world::coordinates::{ChunkLayout, WorldPosition};
use crate::world::is_unit_alive;

use super::id::ProjectileId;
use super::record::{ProjectileRecord, ProjectileStatus};
use super::report::{ProjectileEvent, ProjectileReport, ProjectileTrace};

/// Distance below which an in-flight projectile counts as having reached its target.
const PROJECTILE_IMPACT_DISTANCE_METERS: f32 = 0.1;

/// Advance all in-flight projectiles deterministically by [`ProjectileId`].
///
/// Projectiles whose ids appear in `skip_projectile_ids` are not stepped — used
/// to prevent same-tick movement for projectiles spawned during strike resolution.
pub fn step_all_projectiles(
    world: &mut WorldData,
    delta_seconds: f32,
    skip_projectile_ids: &[ProjectileId],
) -> ProjectileReport {
    let layout = world.layout();
    let ids = world.sorted_projectile_ids();
    let mut report = ProjectileReport::default();
    for id in ids {
        if skip_projectile_ids.contains(&id) {
            continue;
        }
        step_projectile(world, id, delta_seconds, layout, &mut report);
    }
    report
}

fn step_projectile(
    world: &mut WorldData,
    id: ProjectileId,
    delta_seconds: f32,
    layout: ChunkLayout,
    report: &mut ProjectileReport,
) {
    let Some(record) = world.get_projectile(id).cloned() else {
        return;
    };
    if record.status != ProjectileStatus::InFlight {
        return;
    }

    let trace_base = || ProjectileTrace {
        projectile_id: id,
        source_unit_id: record.source_unit_id,
        target_unit_id: record.target_unit_id,
        weapon_id: record.weapon_id.clone(),
        event: ProjectileEvent::Expired,
    };

    let target_alive = world
        .get_unit(record.target_unit_id)
        .is_some_and(is_unit_alive);
    if !target_alive {
        expire_projectile(world, id, report, trace_base());
        return;
    }

    let target_position = world
        .get_unit(record.target_unit_id)
        .map(|unit| unit.placement.position)
        .unwrap_or(record.target_position_snapshot);

    if delta_seconds <= 0.0 {
        let mut parked = record;
        parked.target_position_snapshot = target_position;
        world.insert_projectile(parked);
        return;
    }

    let from = record.position.to_global(layout);
    let to = target_position.to_global(layout);
    let offset = to - from;
    let distance = offset.length();
    let travel = record.speed_mps * delta_seconds;

    if distance <= PROJECTILE_IMPACT_DISTANCE_METERS || travel >= distance {
        resolve_projectile_impact(world, id, record, target_position, report);
        return;
    }

    let direction = offset / distance;
    let next_global = from + direction * travel;
    let mut moving = record;
    moving.position = WorldPosition::from_global(next_global, layout);
    moving.target_position_snapshot = target_position;
    world.insert_projectile(moving);
}

fn resolve_projectile_impact(
    world: &mut WorldData,
    id: ProjectileId,
    record: ProjectileRecord,
    target_position: WorldPosition,
    report: &mut ProjectileReport,
) {
    let trace = |event: ProjectileEvent| ProjectileTrace {
        projectile_id: id,
        source_unit_id: record.source_unit_id,
        target_unit_id: record.target_unit_id,
        weapon_id: record.weapon_id.clone(),
        event,
    };

    match validate_projectile_impact_target(world, record.target_unit_id, &record.launch_snapshot) {
        Ok(()) => {}
        Err(reason) => {
            reject_projectile(
                world,
                id,
                report,
                trace(ProjectileEvent::ImpactRejected { reason }),
            );
            return;
        }
    }

    let hp_before = world
        .get_unit(record.target_unit_id)
        .map(|unit| unit.vitals.current_hp)
        .unwrap_or(0);
    let damage = record.damage.max(0.0) as u32;
    let vitals = match world.damage_unit(record.target_unit_id, damage) {
        Ok(vitals) => vitals,
        Err(_) => {
            reject_projectile(
                world,
                id,
                report,
                trace(ProjectileEvent::ImpactRejected {
                    reason: ProjectileImpactRejection::TargetMissing,
                }),
            );
            return;
        }
    };
    world.record_kill_attribution(record.target_unit_id, record.source_unit_id, hp_before);

    report.push(trace(ProjectileEvent::Hit));
    report.push(trace(ProjectileEvent::DamageApplied {
        damage: record.damage,
        target_hp_before: hp_before,
        target_hp_after: vitals.current_hp,
    }));

    let mut finished = record;
    finished.status = ProjectileStatus::Hit;
    finished.position = target_position;
    finished.target_position_snapshot = target_position;
    world.insert_projectile(finished);
    world.remove_projectile(id);
}

fn reject_projectile(
    world: &mut WorldData,
    id: ProjectileId,
    report: &mut ProjectileReport,
    trace: ProjectileTrace,
) {
    if let Some(mut record) = world.get_projectile(id).cloned() {
        record.status = ProjectileStatus::Invalidated;
        world.insert_projectile(record);
    }
    world.remove_projectile(id);
    report.push(trace);
}

fn expire_projectile(
    world: &mut WorldData,
    id: ProjectileId,
    report: &mut ProjectileReport,
    trace: ProjectileTrace,
) {
    if let Some(mut record) = world.get_projectile(id).cloned() {
        record.status = ProjectileStatus::Expired;
        world.insert_projectile(record);
    }
    world.remove_projectile(id);
    report.push(trace);
}

/// Spawn an authoritative projectile at strike time (ADR-060 C7).
pub fn spawn_projectile_from_strike(
    world: &mut WorldData,
    record: ProjectileRecord,
    report: &mut ProjectileReport,
) {
    let id = record.id;
    report.push(ProjectileTrace {
        projectile_id: id,
        source_unit_id: record.source_unit_id,
        target_unit_id: record.target_unit_id,
        weapon_id: record.weapon_id.clone(),
        event: ProjectileEvent::Spawned,
    });
    world.insert_projectile(record);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::SIMULATION_TICK_SECONDS;
    use crate::world::{
        AttackTargetingPolicy, BuildingCatalog, BuildingConstructionSettings, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, CombatStrikeEvent, CombatStrikeReport, DamageType,
        DoodadCatalog, FootprintCatalog, Heightfield, HitMode, InteriorProfileCatalog,
        LocalPosition, NavigationConfig, PassabilityCatalogs, ProjectileEvent,
        ProjectileImpactRejection, ProjectileLaunchSnapshot, ProjectileReport, TargetFilter,
        UnitDefinitionId, UnitId, UnitOrder, UnitOwnership, UnitSource, WeaponCatalog,
        WeaponDefinition, WeaponDefinitionId, WorldPosition, create_unit_with_ownership,
        default_passability, issue_unit_order, starter_unit_definitions,
        starter_weapon_definitions, step_all_combat_engagement, step_all_combat_strikes,
        step_all_projectiles,
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

    fn catalog() -> crate::world::UnitCatalog {
        crate::world::UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
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

    fn catalog_with_projectile_weapon(weapons: &WeaponCatalog) -> crate::world::UnitCatalog {
        let base = catalog();
        let mut wolf = base.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        wolf.default_weapon_id = WeaponDefinitionId::new("weapon_test_bow");
        crate::world::UnitCatalog::from_definitions(vec![
            wolf,
            base.get(&UnitDefinitionId::new("bandit")).unwrap().clone(),
        ])
        .unwrap()
    }

    fn spawn_player(
        world: &mut WorldData,
        catalog: &crate::world::UnitCatalog,
        x: f32,
        z: f32,
    ) -> UnitId {
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

    fn spawn_hostile(
        world: &mut WorldData,
        catalog: &crate::world::UnitCatalog,
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

    fn step_strikes(
        world: &mut WorldData,
        catalog: &crate::world::UnitCatalog,
        weapons: &WeaponCatalog,
        delta: f32,
    ) -> (crate::world::CombatStrikeReport, ProjectileReport) {
        let mut projectile_spawn = ProjectileReport::default();
        let strikes = step_all_combat_strikes(
            world,
            catalog,
            weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            delta,
            &mut projectile_spawn,
        );
        (strikes, projectile_spawn)
    }

    fn step_projectile_movement(world: &mut WorldData, delta: f32) -> ProjectileReport {
        step_all_projectiles(world, delta, &[])
    }

    fn reassign_unit_ownership(world: &mut WorldData, unit_id: UnitId, ownership: UnitOwnership) {
        let mut record = world.remove_unit_by_id(unit_id).expect("unit exists");
        record.owner_id = ownership.owner_id;
        record.team_id = ownership.team_id;
        record.affiliation = ownership.affiliation;
        let chunk = ChunkId::new(record.placement.position.chunk);
        world.insert_unit(chunk, record).unwrap();
    }

    fn move_unit_to(world: &mut WorldData, unit_id: UnitId, x: f32, z: f32) {
        let mut record = world.remove_unit_by_id(unit_id).expect("unit exists");
        record.placement.position = pos(x, z);
        let chunk = ChunkId::new(record.placement.position.chunk);
        world.insert_unit(chunk, record).unwrap();
    }

    fn spawn_and_strike_projectile(
        world: &mut WorldData,
        catalog: &crate::world::UnitCatalog,
        weapons: &WeaponCatalog,
        player: UnitId,
        hostile: UnitId,
    ) {
        issue_unit_order(
            world,
            catalog,
            weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        step_all_combat_engagement(
            world,
            catalog,
            weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        step_strikes(world, catalog, weapons, 0.2);
    }

    fn neutral_only_projectile_weapon(speed_mps: f32) -> WeaponCatalog {
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
            vec![TargetFilter::Neutral],
            None,
            true,
        ));
        WeaponCatalog::from_definitions(defs).unwrap()
    }

    fn catalog_with_neutral_weapon(_weapons: &WeaponCatalog) -> crate::world::UnitCatalog {
        let base = catalog();
        let mut wolf = base.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        wolf.default_weapon_id = WeaponDefinitionId::new("weapon_test_bow");
        crate::world::UnitCatalog::from_definitions(vec![
            wolf,
            base.get(&UnitDefinitionId::new("deer")).unwrap().clone(),
        ])
        .unwrap()
    }

    fn spawn_neutral(
        world: &mut WorldData,
        catalog: &crate::world::UnitCatalog,
        x: f32,
        z: f32,
    ) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("deer"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::neutral(),
        )
        .unwrap()
        .id
    }

    fn step_combat_with_projectiles(
        world: &mut WorldData,
        catalog: &crate::world::UnitCatalog,
        weapons: &WeaponCatalog,
        delta: f32,
    ) -> (crate::world::CombatStrikeReport, ProjectileReport) {
        let (mut strikes, mut projectile_spawn) = step_strikes(world, catalog, weapons, delta);
        let mut projectiles = step_projectile_movement(world, delta);
        projectiles.traces.append(&mut projectile_spawn.traces);
        step_all_combat_engagement(
            world,
            catalog,
            weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut strikes,
        );
        (strikes, projectiles)
    }

    #[test]
    fn projectile_weapon_spawns_record_at_strike() {
        let weapons = projectile_weapon(20.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 14.0, 10.0);
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
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        let (_, spawn_report) = step_strikes(&mut world, &catalog, &weapons, 0.2);
        assert!(spawn_report.has_event(&ProjectileEvent::Spawned));
        assert_eq!(world.projectiles().count(), 1);
    }

    #[test]
    fn projectile_weapon_does_not_apply_immediate_damage() {
        let weapons = projectile_weapon(5.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 14.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
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
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        let (strike_report, _) = step_strikes(&mut world, &catalog, &weapons, 0.2);
        assert!(
            !strike_report.has_event(&CombatStrikeEvent::AttackStrikeApplied {
                damage: 5.0,
                target_hp_before: hp_before,
                target_hp_after: hp_before.saturating_sub(5),
            })
        );
        assert_eq!(
            world.get_unit(hostile).unwrap().vitals.current_hp,
            hp_before
        );
    }

    #[test]
    fn projectile_moves_toward_target_deterministically() {
        let weapons = projectile_weapon(10.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world_a = flat_world();
        let mut world_b = flat_world();
        for world in [&mut world_a, &mut world_b] {
            let player = spawn_player(world, &catalog, 10.0, 10.0);
            let hostile = spawn_hostile(world, &catalog, 14.0, 10.0);
            issue_unit_order(
                world,
                &catalog,
                &weapons,
                &DoodadCatalog::default(),
                &NavigationConfig::default(),
                player,
                UnitOrder::Attack { target: hostile },
                policy(),
            )
            .unwrap();
            step_all_combat_engagement(
                world,
                &catalog,
                &weapons,
                default_passability(),
                &NavigationConfig::default(),
                policy(),
                &mut CombatStrikeReport::default(),
            );
            step_strikes(world, &catalog, &weapons, 0.2);
        }
        step_projectile_movement(&mut world_a, 0.1);
        step_projectile_movement(&mut world_b, 0.1);
        let pos_a = world_a.projectiles().next().unwrap().1.position;
        let pos_b = world_b.projectiles().next().unwrap().1.position;
        assert_eq!(pos_a, pos_b);
        assert!(pos_a.local.0.x > 10.0);
    }

    #[test]
    fn projectile_reaches_target_and_applies_damage() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
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
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        let (_, spawn_report) = step_strikes(&mut world, &catalog, &weapons, 0.2);
        assert!(spawn_report.has_event(&ProjectileEvent::Spawned));
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.has_event(&ProjectileEvent::Hit));
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                ProjectileEvent::DamageApplied {
                    damage: 5.0,
                    target_hp_before,
                    target_hp_after,
                } if target_hp_before == hp_before && target_hp_after == hp_before.saturating_sub(5)
            )
        }));
        assert_eq!(
            world.get_unit(hostile).unwrap().vitals.current_hp,
            hp_before.saturating_sub(5)
        );
    }

    #[test]
    fn stored_damage_used_even_if_weapon_changes_later() {
        let mut weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
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
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        step_strikes(&mut world, &catalog, &weapons, 0.2);
        let mut changed_defs = starter_weapon_definitions();
        changed_defs.push(WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_test_bow"),
            "Test Bow",
            "Test",
            99.0,
            DamageType::Piercing,
            15.0,
            1.0,
            0.1,
            0.1,
            HitMode::Projectile,
            Some("arrow".to_string()),
            100.0,
            "attack_bow",
            vec![TargetFilter::Enemies],
            None,
            true,
        ));
        let weapons = WeaponCatalog::from_definitions(changed_defs).unwrap();
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                ProjectileEvent::DamageApplied {
                    damage: 5.0,
                    target_hp_before,
                    target_hp_after,
                } if target_hp_before == hp_before && target_hp_after == hp_before.saturating_sub(5)
            )
        }));
    }

    #[test]
    fn projectile_expires_if_target_dies_before_impact() {
        let weapons = projectile_weapon(5.0);
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
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        step_strikes(&mut world, &catalog, &weapons, 0.2);
        world.damage_unit(hostile, 999).unwrap();
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.has_event(&ProjectileEvent::Expired));
        assert_eq!(world.projectiles().count(), 0);
        assert_eq!(world.get_unit(hostile).unwrap().vitals.current_hp, 0);
    }

    #[test]
    fn projectile_continues_if_source_dies_after_launch() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
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
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        step_strikes(&mut world, &catalog, &weapons, 0.2);
        world.damage_unit(player, 999).unwrap();
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.has_event(&ProjectileEvent::Hit));
        assert_eq!(
            world.get_unit(hostile).unwrap().vitals.current_hp,
            hp_before.saturating_sub(5)
        );
    }

    #[test]
    fn projectile_removed_after_hit() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
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
            UnitOrder::Attack { target: hostile },
            policy(),
        )
        .unwrap();
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            default_passability(),
            &NavigationConfig::default(),
            policy(),
            &mut CombatStrikeReport::default(),
        );
        step_strikes(&mut world, &catalog, &weapons, 0.2);
        step_projectile_movement(&mut world, 0.2);
        assert_eq!(world.projectiles().count(), 0);
    }

    #[test]
    fn projectile_id_allocation_is_monotonic() {
        let mut world = flat_world();
        let a = world.allocate_projectile_id();
        let b = world.allocate_projectile_id();
        let c = world.allocate_projectile_id();
        assert!(a.raw() < b.raw());
        assert!(b.raw() < c.raw());
    }

    #[test]
    fn impact_rejects_target_on_same_team_before_impact() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
        spawn_and_strike_projectile(&mut world, &catalog, &weapons, player, hostile);
        reassign_unit_ownership(&mut world, hostile, UnitOwnership::player_default());
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.has_event(&ProjectileEvent::ImpactRejected {
            reason: ProjectileImpactRejection::TargetNowFriendly,
        }));
        assert_eq!(world.projectiles().count(), 0);
        assert_eq!(
            world.get_unit(hostile).unwrap().vitals.current_hp,
            hp_before
        );
    }

    #[test]
    fn impact_rejects_when_weapon_filter_no_longer_matches() {
        let weapons = neutral_only_projectile_weapon(100.0);
        let catalog = catalog_with_neutral_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let neutral = spawn_neutral(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(neutral).unwrap().vitals.current_hp;
        spawn_and_strike_projectile(&mut world, &catalog, &weapons, player, neutral);
        reassign_unit_ownership(&mut world, neutral, UnitOwnership::hostile());
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.has_event(&ProjectileEvent::ImpactRejected {
            reason: ProjectileImpactRejection::TargetFilterRejected,
        }));
        assert_eq!(world.projectiles().count(), 0);
        assert_eq!(
            world.get_unit(neutral).unwrap().vitals.current_hp,
            hp_before
        );
    }

    #[test]
    fn impact_rejects_removed_target_during_travel() {
        let weapons = projectile_weapon(5.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 20.0, 10.0);
        spawn_and_strike_projectile(&mut world, &catalog, &weapons, player, hostile);
        world.remove_unit_by_id(hostile);
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.has_event(&ProjectileEvent::Expired));
        assert_eq!(world.projectiles().count(), 0);
    }

    #[test]
    fn source_removed_after_launch_still_damages_valid_target() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
        spawn_and_strike_projectile(&mut world, &catalog, &weapons, player, hostile);
        world.remove_unit_by_id(player);
        let report = step_projectile_movement(&mut world, 0.2);
        assert!(report.has_event(&ProjectileEvent::Hit));
        assert_eq!(
            world.get_unit(hostile).unwrap().vitals.current_hp,
            hp_before.saturating_sub(5)
        );
    }

    #[test]
    fn target_beyond_weapon_range_still_hits_if_otherwise_valid() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
        spawn_and_strike_projectile(&mut world, &catalog, &weapons, player, hostile);
        move_unit_to(&mut world, hostile, 100.0, 10.0);
        let report = step_projectile_movement(&mut world, 2.0);
        assert!(report.has_event(&ProjectileEvent::Hit));
        assert_eq!(
            world.get_unit(hostile).unwrap().vitals.current_hp,
            hp_before.saturating_sub(5)
        );
    }

    #[test]
    fn rejected_impact_removes_projectile_with_deterministic_reason() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        spawn_and_strike_projectile(&mut world, &catalog, &weapons, player, hostile);
        reassign_unit_ownership(&mut world, hostile, UnitOwnership::player_default());
        let report = step_projectile_movement(&mut world, 0.2);
        let rejections: Vec<_> = report
            .traces
            .iter()
            .filter_map(|trace| match trace.event {
                ProjectileEvent::ImpactRejected { reason } => Some(reason),
                _ => None,
            })
            .collect();
        assert_eq!(
            rejections,
            vec![ProjectileImpactRejection::TargetNowFriendly]
        );
        assert_eq!(world.projectiles().count(), 0);
    }

    #[test]
    fn projectile_iteration_is_deterministic() {
        let mut world = flat_world();
        let source = UnitId::new(1);
        let target = UnitId::new(2);
        let weapon_id = WeaponDefinitionId::new("weapon_test_bow");
        for id in [3u64, 1, 2] {
            world.insert_projectile(ProjectileRecord::new_in_flight(
                ProjectileId::new(id),
                source,
                target,
                weapon_id.clone(),
                1.0,
                DamageType::Piercing,
                pos(0.0, 0.0),
                pos(1.0, 0.0),
                10.0,
                crate::world::ProjectileLaunchSnapshot::render_test_placeholder(source),
            ));
        }
        let ids: Vec<_> = world.sorted_projectile_ids();
        assert_eq!(
            ids,
            vec![
                ProjectileId::new(1),
                ProjectileId::new(2),
                ProjectileId::new(3)
            ]
        );
    }

    #[test]
    fn integrated_movement_tick_steps_projectiles_before_death() {
        let weapons = projectile_weapon(100.0);
        let catalog = catalog_with_projectile_weapon(&weapons);
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
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
        for _ in 0..10 {
            let mut scan = crate::world::CombatAiScanState::default();
            let settings = crate::world::CombatAiSettings::default();
            crate::simulation::run_simulation_tick(
                &mut world,
                &catalog,
                &weapons,
                &DoodadCatalog::default(),
                &BuildingCatalog::default(),
                &FootprintCatalog::default(),
                &crate::world::BuildingInteractionProfileCatalog::default(),
                &NavigationConfig::default(),
                policy(),
                &settings,
                &mut scan,
                BuildingConstructionSettings::default(),
                &InteriorProfileCatalog::default(),
                &crate::world::ItemCatalog::default(),
                &crate::world::ItemCategoryCatalog::default(),
                &crate::world::InventoryProfileCatalog::default(),
                &crate::world::CorpseSettings::default(),
                SIMULATION_TICK_SECONDS,
                0,
                None,
            );
        }
        if let Some(record) = world.get_unit(hostile) {
            assert_eq!(record.vitals.current_hp, hp_before.saturating_sub(5));
        } else {
            assert!(
                hp_before <= 5,
                "unit removed only after lethal projectile damage"
            );
        }
    }
}
