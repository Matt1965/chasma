//! Unit orders and issuance (ADR-030 U5, ADR-032 U7, ADR-037 U12, ADR-056 C3).

use bevy::prelude::*;

use super::catalog::UnitCatalog;
use super::combat_state::CombatState;
use super::id::UnitId;
use super::state::UnitState;
use crate::world::movement::feel::PATH_RESOLVE_BUDGET_PER_TICK;
use crate::world::task::{TaskCancelReason, cancel_unit_task};
use crate::world::unit::unit_can_execute_actions;
use crate::world::{
    AttackTargetingPolicy, CommandBufferResolveReport, CommandResolveSuccess, DoodadCatalog,
    NavigationConfig, PassabilityCatalogs, WeaponCatalog, WorldData, WorldPosition,
    clear_attack_cycle_for_order_cancel, initial_attack_combat_state,
    reset_attack_cycle_for_retarget, validate_attack_target,
};

/// Authoritative command issued to a unit instance.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum UnitOrder {
    Idle,
    MoveTo {
        target: WorldPosition,
    },
    /// Attack a specific unit (ADR-056 C3+).
    Attack {
        target: UnitId,
    },
    /// Move while attack-scanning (ADR-057).
    AttackMove {
        destination: WorldPosition,
    },
    /// Travel to and perform an assigned work task (ADR-085 B8).
    Work {
        task_id: crate::world::TaskId,
        target: WorldPosition,
    },
}

/// Why [`issue_unit_order`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitOrderError {
    UnitNotFound,
    DefinitionNotFound,
    PathStartBlocked,
    PathGoalBlocked,
    NoPath,
    PathTerrainUnavailable,
    AttackerNotFound,
    TargetNotFound,
    SelfTarget,
    AttackerDead,
    TargetDead,
    MissingWeapon,
    InvalidOwnershipTarget,
    WeaponCannotTarget,
}

impl std::fmt::Display for UnitOrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnitNotFound => write!(f, "unit not found"),
            Self::DefinitionNotFound => write!(f, "definition not found"),
            Self::PathStartBlocked => write!(f, "path start blocked"),
            Self::PathGoalBlocked => write!(f, "path goal blocked"),
            Self::NoPath => write!(f, "no path"),
            Self::PathTerrainUnavailable => write!(f, "terrain unavailable"),
            Self::AttackerNotFound => write!(f, "attacker not found"),
            Self::TargetNotFound => write!(f, "target not found"),
            Self::SelfTarget => write!(f, "cannot attack self"),
            Self::AttackerDead => write!(f, "attacker dead"),
            Self::TargetDead => write!(f, "target dead"),
            Self::MissingWeapon => write!(f, "missing weapon"),
            Self::InvalidOwnershipTarget => write!(f, "invalid ownership target"),
            Self::WeaponCannotTarget => write!(f, "weapon cannot target"),
        }
    }
}

/// Issue an order to a unit.
///
/// `MoveTo` is queued on the command buffer and resolved before the next movement
/// step. `Idle`, `Attack`, and `AttackMove` apply immediately.
pub fn issue_unit_order(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    order: UnitOrder,
    targeting_policy: AttackTargetingPolicy,
) -> Result<(), UnitOrderError> {
    if world.get_unit(unit_id).is_none() {
        return Err(UnitOrderError::UnitNotFound);
    }
    if !unit_can_execute_actions(world, unit_id) {
        return Err(UnitOrderError::UnitNotFound);
    }
    match order {
        UnitOrder::Idle => {
            let mut events = Vec::new();
            cancel_unit_task(world, unit_id, TaskCancelReason::PlayerOrder, &mut events);
            let _ = events;
            world.command_buffer_mut().clear_pending(unit_id);
            world.movement_smoothing_mut().clear_unit(unit_id);
            world
                .set_unit_state(unit_id, UnitState::Idle)
                .map_err(|_| UnitOrderError::UnitNotFound)?;
            clear_attack_cycle_for_order_cancel(world, unit_id, None, unit_catalog, weapon_catalog);
            world
                .set_unit_combat_state(unit_id, CombatState::Peaceful)
                .map_err(|_| UnitOrderError::UnitNotFound)?;
            Ok(())
        }
        UnitOrder::MoveTo { .. } => {
            let mut events = Vec::new();
            cancel_unit_task(world, unit_id, TaskCancelReason::PlayerOrder, &mut events);
            let _ = events;
            let _ = (weapon_catalog, targeting_policy);
            if world.get_unit(unit_id).is_none() {
                return Err(UnitOrderError::UnitNotFound);
            }
            world.command_buffer_mut().clear_pending(unit_id);
            world.movement_smoothing_mut().clear_unit(unit_id);
            world
                .set_unit_combat_state(unit_id, CombatState::Peaceful)
                .map_err(|_| UnitOrderError::UnitNotFound)?;
            clear_attack_cycle_for_order_cancel(world, unit_id, None, unit_catalog, weapon_catalog);
            world.command_buffer_mut().enqueue(unit_id, order);
            Ok(())
        }
        UnitOrder::Attack { target } => {
            let mut events = Vec::new();
            cancel_unit_task(world, unit_id, TaskCancelReason::PlayerOrder, &mut events);
            let _ = events;
            let _ = (doodad_catalog, nav_config);
            validate_attack_target(
                world,
                unit_id,
                target,
                weapon_catalog,
                unit_catalog,
                targeting_policy,
            )?;
            let old_cycle_target = world
                .get_unit(unit_id)
                .and_then(|record| record.attack_cycle.as_ref().map(|cycle| cycle.target));
            if old_cycle_target.is_some_and(|old| old != target) {
                reset_attack_cycle_for_retarget(
                    world,
                    unit_id,
                    old_cycle_target.unwrap(),
                    target,
                    None,
                    unit_catalog,
                    weapon_catalog,
                );
            }
            world.command_buffer_mut().clear_pending(unit_id);
            world.movement_smoothing_mut().clear_unit(unit_id);
            let combat_state =
                initial_attack_combat_state(world, unit_id, target, unit_catalog, weapon_catalog);
            world
                .set_unit_combat_state(unit_id, combat_state)
                .map_err(|_| UnitOrderError::AttackerNotFound)?;
            Ok(())
        }
        UnitOrder::AttackMove { destination } => {
            let mut events = Vec::new();
            cancel_unit_task(world, unit_id, TaskCancelReason::PlayerOrder, &mut events);
            let _ = events;
            let _ = (
                doodad_catalog,
                nav_config,
                weapon_catalog,
                unit_catalog,
                targeting_policy,
            );
            if world.get_unit(unit_id).is_none() {
                return Err(UnitOrderError::AttackerNotFound);
            }
            world.command_buffer_mut().clear_pending(unit_id);
            world.movement_smoothing_mut().clear_unit(unit_id);
            clear_attack_cycle_for_order_cancel(world, unit_id, None, unit_catalog, weapon_catalog);
            world
                .set_unit_combat_state(
                    unit_id,
                    CombatState::AttackMoving {
                        destination,
                        target: None,
                    },
                )
                .map_err(|_| UnitOrderError::AttackerNotFound)?;
            Ok(())
        }
        UnitOrder::Work { .. } => {
            if world.get_unit(unit_id).is_none() {
                return Err(UnitOrderError::UnitNotFound);
            }
            world.command_buffer_mut().clear_pending(unit_id);
            world.movement_smoothing_mut().clear_unit(unit_id);
            world
                .set_unit_combat_state(unit_id, CombatState::Peaceful)
                .map_err(|_| UnitOrderError::UnitNotFound)?;
            clear_attack_cycle_for_order_cancel(world, unit_id, None, unit_catalog, weapon_catalog);
            world.command_buffer_mut().enqueue(unit_id, order);
            Ok(())
        }
    }
}

/// Resolve deferred orders before movement (ADR-037 U12).
///
/// Processes at most [`PATH_RESOLVE_BUDGET_PER_TICK`] paths per call so large
/// group moves do not stall a single frame.
pub fn resolve_pending_unit_orders(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
) -> CommandBufferResolveReport {
    if world.command_buffer().is_empty() {
        return CommandBufferResolveReport::default();
    }

    let batch = world
        .command_buffer_mut()
        .drain_sorted_budget(PATH_RESOLVE_BUDGET_PER_TICK);
    let mut report = CommandBufferResolveReport::default();
    for entry in batch {
        match crate::world::movement::feel::resolve_one(
            world,
            unit_catalog,
            catalogs,
            nav_config,
            entry.unit_id,
            entry.order,
        ) {
            Ok(()) => {
                report.resolved += 1;
                if let Some(record) = world.get_unit(entry.unit_id) {
                    if let UnitState::Moving {
                        target, ref path, ..
                    } = record.state
                    {
                        report.successes.push(CommandResolveSuccess {
                            unit_id: entry.unit_id,
                            target,
                            path_waypoint_count: path.len() as u32,
                        });
                    }
                }
            }
            Err(error) => {
                report.failed += 1;
                report.failures.push((entry.unit_id, error));
            }
        }
    }
    report
}

/// Resolve every queued order (for tests and tooling).
pub fn resolve_all_pending_unit_orders(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
) -> CommandBufferResolveReport {
    let mut total = CommandBufferResolveReport::default();
    while !world.command_buffer().is_empty() {
        let batch = resolve_pending_unit_orders(world, unit_catalog, catalogs, nav_config);
        if batch.resolved == 0 && batch.failed == 0 {
            break;
        }
        total.resolved += batch.resolved;
        total.failed += batch.failed;
        total.failures.extend(batch.failures);
        total.successes.extend(batch.successes);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        AttackTargetingPolicy, BuildingCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        DoodadCatalog, FootprintCatalog, Heightfield, LocalPosition, PassabilityCatalogs,
        UnitDefinitionId, UnitOwnership, UnitSource, WeaponCatalog, create_unit,
        create_unit_with_ownership, default_passability, resolve_pending_unit_orders,
    };

    fn layout_world() -> WorldData {
        WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn insert_flat(world: &mut WorldData) {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn weapons() -> WeaponCatalog {
        WeaponCatalog::default()
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    fn issue(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        unit_id: UnitId,
        order: UnitOrder,
    ) -> Result<(), UnitOrderError> {
        issue_unit_order(
            world,
            catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            unit_id,
            order,
            policy(),
        )
    }

    #[test]
    fn issue_move_to_queues_then_resolves_with_path() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        insert_flat(&mut world);
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let target = pos(100.0, 50.0);
        issue(&mut world, &catalog, unit_id, UnitOrder::MoveTo { target }).unwrap();
        assert_eq!(world.get_unit(unit_id).unwrap().state, UnitState::Idle);
        assert!(world.command_buffer().pending_for(unit_id).is_some());

        resolve_pending_unit_orders(
            &mut world,
            &catalog,
            default_passability(),
            &NavigationConfig::default(),
        );

        let record = world.get_unit(unit_id).unwrap();
        match record.state {
            UnitState::Moving {
                target: stored_target,
                ref path,
                waypoint_index,
            } => {
                assert_eq!(stored_target, target);
                assert!(!path.is_empty());
                assert_eq!(waypoint_index, 0);
            }
            _ => panic!("expected Moving state"),
        }
        assert_eq!(record.placement.position, pos(10.0, 10.0));
    }

    #[test]
    fn valid_attack_order_sets_combat_state() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;

        issue(
            &mut world,
            &catalog,
            player,
            UnitOrder::Attack { target: hostile },
        )
        .unwrap();
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::Attacking { target: t } | CombatState::Chasing { target: t } if t == hostile
        ));
    }

    #[test]
    fn invalid_attack_order_does_not_mutate_state() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let before = world.get_unit(player).unwrap().combat_state.clone();

        let err = issue(
            &mut world,
            &catalog,
            player,
            UnitOrder::Attack { target: player },
        )
        .unwrap_err();
        assert_eq!(err, UnitOrderError::SelfTarget);
        assert_eq!(world.get_unit(player).unwrap().combat_state, before);
    }

    #[test]
    fn attack_move_stores_destination() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let destination = pos(30.0, 30.0);

        issue(
            &mut world,
            &catalog,
            player,
            UnitOrder::AttackMove { destination },
        )
        .unwrap();
        assert!(matches!(
            world.get_unit(player).unwrap().combat_state,
            CombatState::AttackMoving {
                destination: d,
                target: None
            } if d == destination
        ));
    }

    #[test]
    fn issue_order_missing_unit() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        insert_flat(&mut world);
        let err = issue(
            &mut world,
            &catalog,
            UnitId::new(1),
            UnitOrder::MoveTo {
                target: pos(1.0, 1.0),
            },
        )
        .unwrap_err();
        assert_eq!(err, UnitOrderError::UnitNotFound);
    }
}
