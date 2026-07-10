//! Unit death detection, deferred removal queue, and target cleanup (ADR-059 C6).

use std::collections::HashSet;

use bevy::prelude::*;

use super::combat_state::CombatState;
use super::id::UnitId;
use super::state::UnitState;
use crate::world::WorldData;

/// Why a unit was queued for removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum RemovalReason {
    Killed,
    DevDeleted,
    Cleanup,
    Unknown,
}

/// Latest lethal strike attribution for death traces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KillAttribution {
    pub killer: UnitId,
    pub hp_before: u32,
}

/// One deferred removal request.
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct UnitRemovalEntry {
    pub unit_id: UnitId,
    pub reason: RemovalReason,
    pub killer: Option<UnitId>,
    pub tick: u64,
}

/// Deferred removal queue — units are not erased during damage iteration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitRemovalQueue {
    entries: Vec<UnitRemovalEntry>,
    queued_ids: HashSet<UnitId>,
}

impl UnitRemovalQueue {
    pub fn entries(&self) -> &[UnitRemovalEntry] {
        &self.entries
    }

    pub fn is_queued(&self, unit_id: UnitId) -> bool {
        self.queued_ids.contains(&unit_id)
    }

    pub fn queue(&mut self, entry: UnitRemovalEntry) -> bool {
        if self.queued_ids.insert(entry.unit_id) {
            self.entries.push(entry);
            true
        } else {
            false
        }
    }

    pub fn drain(&mut self) -> Vec<UnitRemovalEntry> {
        self.queued_ids.clear();
        std::mem::take(&mut self.entries)
    }
}

/// Combat death trace events (ADR-059 C6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitDeathEvent {
    UnitDied {
        hp_before: u32,
        hp_after: u32,
        killer: Option<UnitId>,
        tick: u64,
    },
    UnitRemovalQueued {
        reason: RemovalReason,
        killer: Option<UnitId>,
        tick: u64,
    },
    UnitRemoved {
        reason: RemovalReason,
        killer: Option<UnitId>,
        tick: u64,
    },
    TargetClearedDueToDeath {
        cleared_target: UnitId,
    },
}

/// One death pipeline trace row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitDeathTrace {
    pub unit_id: UnitId,
    pub event: UnitDeathEvent,
}

/// Aggregated death tick report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitDeathReport {
    pub traces: Vec<UnitDeathTrace>,
    pub removed_unit_ids: Vec<UnitId>,
}

impl UnitDeathReport {
    pub fn push(&mut self, trace: UnitDeathTrace) {
        self.traces.push(trace);
    }
}

/// Record the latest attacker for kill attribution at death time.
pub fn record_kill_attribution(
    world: &mut WorldData,
    target_id: UnitId,
    attacker_id: UnitId,
    hp_before: u32,
) {
    world.record_kill_attribution(target_id, attacker_id, hp_before);
}

/// Queue a unit for deferred removal (dev/delete APIs reuse this).
pub fn queue_unit_removal(
    world: &mut WorldData,
    unit_id: UnitId,
    reason: RemovalReason,
    killer: Option<UnitId>,
    tick: u64,
) -> bool {
    world.queue_unit_removal(UnitRemovalEntry {
        unit_id,
        reason,
        killer,
        tick,
    })
}

/// Death pipeline: detect → mark dead → queue → target cleanup → remove.
pub fn step_unit_death_pipeline(world: &mut WorldData, tick: u64) -> UnitDeathReport {
    let mut report = UnitDeathReport::default();
    detect_and_queue_deaths(world, tick, &mut report);
    let pending_removals = clear_targets_for_dead_units(world, &mut report);
    apply_removals(world, pending_removals, &mut report);
    report
}

fn detect_and_queue_deaths(world: &mut WorldData, tick: u64, report: &mut UnitDeathReport) {
    let candidates: Vec<UnitId> = world
        .sorted_unit_ids()
        .into_iter()
        .filter(|unit_id| {
            world
                .get_unit(*unit_id)
                .map(|record| record.vitals.current_hp == 0 && record.state != UnitState::Dead)
                .unwrap_or(false)
        })
        .collect();

    for unit_id in candidates {
        let attribution = world.take_kill_attribution_info(unit_id);
        let killer = attribution.map(|info| info.killer);
        let hp_before = attribution.map(|info| info.hp_before).unwrap_or(0);

        let _ = world.set_unit_state(unit_id, UnitState::Dead);
        finalize_dead_unit_side_effects(world, unit_id);

        report.push(UnitDeathTrace {
            unit_id,
            event: UnitDeathEvent::UnitDied {
                hp_before,
                hp_after: 0,
                killer,
                tick,
            },
        });

        if queue_unit_removal(world, unit_id, RemovalReason::Killed, killer, tick) {
            report.push(UnitDeathTrace {
                unit_id,
                event: UnitDeathEvent::UnitRemovalQueued {
                    reason: RemovalReason::Killed,
                    killer,
                    tick,
                },
            });
        }
    }
}

fn finalize_dead_unit_side_effects(world: &mut WorldData, unit_id: UnitId) {
    world.command_buffer_mut().clear_pending(unit_id);
    world.movement_smoothing_mut().clear_unit(unit_id);
    let _ = world.set_unit_attack_cycle(unit_id, None);
    let _ = world.set_unit_combat_state(unit_id, CombatState::Peaceful);
}

fn clear_targets_for_dead_units(
    world: &mut WorldData,
    report: &mut UnitDeathReport,
) -> Vec<UnitRemovalEntry> {
    let dead_targets: HashSet<UnitId> = world
        .sorted_unit_ids()
        .into_iter()
        .filter(|unit_id| {
            world
                .get_unit(*unit_id)
                .map(|record| {
                    record.vitals.current_hp == 0
                        || matches!(record.state, UnitState::Dead)
                        || world.removal_queue().is_queued(*unit_id)
                })
                .unwrap_or(false)
        })
        .collect();

    if dead_targets.is_empty() {
        return world.removal_queue_mut().drain();
    }

    for unit_id in world.sorted_unit_ids() {
        if dead_targets.contains(&unit_id) {
            continue;
        }
        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        let combat_state = record.combat_state.clone();
        let cleared_target = match &combat_state {
            CombatState::Attacking { target } | CombatState::Chasing { target } => {
                dead_targets.contains(target).then_some(*target)
            }
            CombatState::AttackMoving {
                target: Some(target),
                ..
            } => dead_targets.contains(target).then_some(*target),
            _ => None,
        };

        let Some(cleared_target) = cleared_target else {
            continue;
        };

        let next = match combat_state {
            CombatState::AttackMoving { destination, .. } => CombatState::AttackMoving {
                destination,
                target: None,
            },
            _ => CombatState::Peaceful,
        };
        let _ = world.set_unit_combat_state(unit_id, next);
        let _ = world.set_unit_attack_cycle(unit_id, None);
        world.command_buffer_mut().clear_pending(unit_id);
        world.movement_smoothing_mut().clear_unit(unit_id);
        if matches!(world.get_unit(unit_id).map(|r| &r.state), Some(UnitState::Moving { .. })) {
            let _ = world.set_unit_state(unit_id, UnitState::Idle);
        }

        report.push(UnitDeathTrace {
            unit_id,
            event: UnitDeathEvent::TargetClearedDueToDeath { cleared_target },
        });
    }

    world.removal_queue_mut().drain()
}

fn apply_removals(
    world: &mut WorldData,
    entries: Vec<UnitRemovalEntry>,
    report: &mut UnitDeathReport,
) {
    let mut sorted = entries;
    sorted.sort_by_key(|entry| entry.unit_id);

    for entry in sorted {
        let removed = world.remove_unit_by_id(entry.unit_id);
        if removed.is_some() {
            report.removed_unit_ids.push(entry.unit_id);
            report.push(UnitDeathTrace {
                unit_id: entry.unit_id,
                event: UnitDeathEvent::UnitRemoved {
                    reason: entry.reason,
                    killer: entry.killer,
                    tick: entry.tick,
                },
            });
        }
        world.clear_kill_attribution(entry.unit_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::input::SelectedUnits;
    use crate::world::combat::{step_all_combat_engagement, step_all_combat_strikes};
    use crate::world::{
        create_unit_with_ownership, issue_unit_order, starter_unit_definitions,
        starter_weapon_definitions, AttackTargetingPolicy, ChunkCoord, ChunkData, ChunkId,
        ChunkLayout, Heightfield, LocalPosition, UnitCatalog, UnitDefinitionId, UnitOrder,
        UnitOwnership, UnitSource, WeaponCatalog, WorldPosition,
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
        WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
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

    #[test]
    fn hp_reaches_zero_marks_unit_dead() {
        let catalog = catalog();
        let mut world = flat_world();
        let hostile = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        world.damage_unit(hostile, 999).unwrap();
        assert_eq!(world.get_unit(hostile).unwrap().vitals.current_hp, 0);
        assert_ne!(world.get_unit(hostile).unwrap().state, UnitState::Dead);

        let report = step_unit_death_pipeline(&mut world, 1);
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                UnitDeathEvent::UnitDied {
                    hp_after: 0,
                    ..
                }
            )
        }));
    }

    #[test]
    fn dead_unit_queued_for_removal() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        world.damage_unit(unit, 999).unwrap();
        let report = step_unit_death_pipeline(&mut world, 1);
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                UnitDeathEvent::UnitRemovalQueued {
                    reason: RemovalReason::Killed,
                    ..
                }
            )
        }));
    }

    #[test]
    fn removal_queue_removes_unit_from_world_data() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        world.damage_unit(unit, 999).unwrap();
        let report = step_unit_death_pipeline(&mut world, 1);
        assert!(world.get_unit(unit).is_none());
        assert!(report.removed_unit_ids.contains(&unit));
    }

    #[test]
    fn removal_is_deferred_not_immediate_during_damage() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 8).unwrap();
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &crate::world::DoodadCatalog::default(),
            &crate::world::NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            AttackTargetingPolicy::default(),
        )
        .unwrap();
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons(),
            &crate::world::DoodadCatalog::default(),
            &crate::world::NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &mut crate::world::CombatStrikeReport::default(),
        );
        step_all_combat_strikes(
            &mut world,
            &catalog,
            &weapons(),
            &crate::world::DoodadCatalog::default(),
            &crate::world::NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            0.2,
            &mut crate::world::ProjectileReport::default(),
        );
        assert!(world.get_unit(hostile).is_some());
        assert_eq!(world.get_unit(hostile).unwrap().vitals.current_hp, 0);
        assert!(!matches!(
            world.get_unit(hostile).unwrap().state,
            UnitState::Dead
        ));
    }

    #[test]
    fn selected_dead_unit_removed_from_selection() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        let mut selection = SelectedUnits::default();
        selection.set_single(unit);
        world.damage_unit(unit, 999).unwrap();
        step_unit_death_pipeline(&mut world, 1);
        selection.prune_dead(&world);
        assert!(!selection.contains(unit));
    }

    #[test]
    fn attacker_target_cleared_when_target_dies() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &crate::world::DoodadCatalog::default(),
            &crate::world::NavigationConfig::default(),
            player,
            UnitOrder::Attack { target: hostile },
            AttackTargetingPolicy::default(),
        )
        .unwrap();
        world.damage_unit(hostile, 999).unwrap();
        let report = step_unit_death_pipeline(&mut world, 1);
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                UnitDeathEvent::TargetClearedDueToDeath {
                    cleared_target,
                } if cleared_target == hostile
            )
        }));
        assert_eq!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Peaceful
        );
    }

    #[test]
    fn attack_move_acquired_target_cleared_when_target_dies() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &crate::world::DoodadCatalog::default(),
            &crate::world::NavigationConfig::default(),
            player,
            UnitOrder::AttackMove {
                destination: pos(80.0, 80.0),
            },
            AttackTargetingPolicy::default(),
        )
        .unwrap();
        world
            .set_unit_combat_state(
                player,
                CombatState::AttackMoving {
                    destination: pos(80.0, 80.0),
                    target: Some(hostile),
                },
            )
            .unwrap();
        world.damage_unit(hostile, 999).unwrap();
        step_unit_death_pipeline(&mut world, 1);
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::AttackMoving {
                target: None,
                ..
            }
        ));
    }

    #[test]
    fn movement_skips_dead_units() {
        use crate::world::step_unit_movement;
        let catalog = catalog();
        let mut world = flat_world();
        let unit = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        let before = world.get_unit(unit).unwrap().placement.position;
        world
            .set_unit_state(
                unit,
                UnitState::Moving {
                    target: pos(20.0, 10.0),
                    path: crate::world::NavigationPath {
                        waypoints: vec![pos(20.0, 10.0)],
                    },
                    waypoint_index: 0,
                },
            )
            .unwrap();
        world.damage_unit(unit, 999).unwrap();
        world.set_unit_state(unit, UnitState::Dead).unwrap();
        let _ = step_unit_movement(
            &mut world,
            &catalog,
            &crate::world::DoodadCatalog::default(),
            unit,
            0.2,
        );
        let after = world.get_unit(unit).unwrap().placement.position;
        assert_eq!(before, after);
    }

    #[test]
    fn dead_units_cannot_receive_orders() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit = spawn_hostile(&mut world, &catalog, 10.0, 10.0);
        world.damage_unit(unit, 999).unwrap();
        step_unit_death_pipeline(&mut world, 1);
        assert!(world.get_unit(unit).is_none());
        let err = issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &crate::world::DoodadCatalog::default(),
            &crate::world::NavigationConfig::default(),
            unit,
            UnitOrder::Idle,
            AttackTargetingPolicy::default(),
        );
        assert!(matches!(err, Err(crate::world::UnitOrderError::UnitNotFound)));
    }

    #[test]
    fn duplicate_death_queue_entries_are_prevented() {
        let mut world = flat_world();
        let unit = UnitId::new(1);
        assert!(queue_unit_removal(
            &mut world,
            unit,
            RemovalReason::Killed,
            None,
            1
        ));
        assert!(!queue_unit_removal(
            &mut world,
            unit,
            RemovalReason::Killed,
            None,
            2
        ));
    }

    #[test]
    fn killer_unit_id_is_recorded() {
        let catalog = catalog();
        let mut world = flat_world();
        let player = spawn_player(&mut world, &catalog, 10.0, 10.0);
        let hostile = spawn_hostile(&mut world, &catalog, 11.0, 10.0);
        world.set_unit_hp(hostile, 8).unwrap();
        record_kill_attribution(&mut world, hostile, player, 8);
        world.damage_unit(hostile, 8).unwrap();
        let report = step_unit_death_pipeline(&mut world, 1);
        assert!(report.traces.iter().any(|trace| {
            matches!(
                trace.event,
                UnitDeathEvent::UnitDied {
                    killer: Some(killer),
                    ..
                } if killer == player
            )
        }));
    }

    #[test]
    fn deterministic_removal_ordering() {
        let catalog = catalog();
        let mut world_a = flat_world();
        let mut world_b = flat_world();
        let a1 = spawn_hostile(&mut world_a, &catalog, 10.0, 10.0);
        let a2 = spawn_hostile(&mut world_a, &catalog, 12.0, 10.0);
        let b1 = spawn_hostile(&mut world_b, &catalog, 10.0, 10.0);
        let b2 = spawn_hostile(&mut world_b, &catalog, 12.0, 10.0);
        for unit in [a1, a2] {
            world_a.damage_unit(unit, 999).unwrap();
        }
        for unit in [b1, b2] {
            world_b.damage_unit(unit, 999).unwrap();
        }
        let report_a = step_unit_death_pipeline(&mut world_a, 1);
        let report_b = step_unit_death_pipeline(&mut world_b, 1);
        assert_eq!(report_a.removed_unit_ids, report_b.removed_unit_ids);
    }
}
