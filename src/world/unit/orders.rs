//! Unit orders and issuance (ADR-030 U5, ADR-032 U7, ADR-037 U12).

use bevy::prelude::*;

use super::catalog::UnitCatalog;
use super::id::UnitId;
use super::state::UnitState;
use crate::world::{
    CommandBufferResolveReport, CommandResolveSuccess, DoodadCatalog, NavigationConfig, WorldData,
    WorldPosition,
};
use crate::world::movement::feel::PATH_RESOLVE_BUDGET_PER_TICK;

/// Authoritative command issued to a unit instance.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum UnitOrder {
    Idle,
    MoveTo {
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
}

/// Issue an order to a unit.
///
/// `MoveTo` is queued on the command buffer and resolved before the next movement
/// step. `Idle` applies immediately.
pub fn issue_unit_order(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    order: UnitOrder,
) -> Result<(), UnitOrderError> {
    match order {
        UnitOrder::Idle => {
            world.command_buffer_mut().clear_pending(unit_id);
            world.movement_smoothing_mut().clear_unit(unit_id);
            world
                .set_unit_state(unit_id, UnitState::Idle)
                .map_err(|_| UnitOrderError::UnitNotFound)?;
            Ok(())
        }
        UnitOrder::MoveTo { .. } => {
            let _ = (unit_catalog, doodad_catalog, nav_config);
            if world.get_unit(unit_id).is_none() {
                return Err(UnitOrderError::UnitNotFound);
            }
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
    doodad_catalog: &DoodadCatalog,
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
            doodad_catalog,
            nav_config,
            entry.unit_id,
            entry.order,
        ) {
            Ok(()) => {
                report.resolved += 1;
                if let Some(record) = world.get_unit(entry.unit_id) {
                    if let UnitState::Moving {
                        target,
                        ref path,
                        ..
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
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
) -> CommandBufferResolveReport {
    let mut total = CommandBufferResolveReport::default();
    while !world.command_buffer().is_empty() {
        let batch = resolve_pending_unit_orders(world, unit_catalog, doodad_catalog, nav_config);
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
        create_unit, resolve_pending_unit_orders, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        Heightfield, LocalPosition, UnitDefinitionId, UnitSource,
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

    fn issue(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        unit_id: UnitId,
        order: UnitOrder,
    ) -> Result<(), UnitOrderError> {
        issue_unit_order(
            world,
            catalog,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            unit_id,
            order,
        )
    }

    fn issue_and_resolve(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        unit_id: UnitId,
        order: UnitOrder,
    ) -> Result<(), UnitOrderError> {
        issue(world, catalog, unit_id, order)?;
        let report = resolve_pending_unit_orders(
            world,
            catalog,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
        );
        if report.failed > 0 {
            return Err(report.failures[0].1);
        }
        Ok(())
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
            &DoodadCatalog::default(),
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
    fn no_movement_state_before_buffer_resolution() {
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

        issue(
            &mut world,
            &catalog,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(100.0, 50.0),
            },
        )
        .unwrap();
        assert!(!matches!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Moving { .. }
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

    #[test]
    fn issue_move_without_terrain_fails_on_resolve() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        issue(
            &mut world,
            &catalog,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(100.0, 50.0),
            },
        )
        .unwrap();
        let report = resolve_pending_unit_orders(
            &mut world,
            &catalog,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
        );
        assert_eq!(report.failed, 1);
        assert_eq!(report.failures[0].1, UnitOrderError::PathTerrainUnavailable);
    }
}
