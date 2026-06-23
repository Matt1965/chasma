//! Multi-unit command dispatch (ADR-034 U9, ADR-035 U10).

use bevy::prelude::*;

use crate::world::{
    issue_unit_order, DoodadCatalog, FormationKind, FormationPlanner, NavigationConfig, UnitCatalog,
    UnitOrder, UnitOrderError, WorldData, WorldPosition,
};

use super::selection::SelectedUnits;

/// Outcome of issuing a move order to every selected unit.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MoveOrdersReport {
    pub issued: u32,
    pub failed: u32,
    pub unit_traces: Vec<MoveOrderUnitTrace>,
}

/// Per-unit move order trace for command observability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveOrderUnitTrace {
    pub unit_id: crate::world::UnitId,
    pub order: UnitOrder,
    pub error: Option<UnitOrderError>,
}

/// Issue formation-distributed `MoveTo` orders for each selected unit.
///
/// Does not mutate selection or bypass [`issue_unit_order`].
pub fn issue_move_orders_to_selection(
    world: &mut WorldData,
    selection: &SelectedUnits,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    target: WorldPosition,
) -> MoveOrdersReport {
    let unit_ids: Vec<_> = selection.iter().collect();
    if unit_ids.is_empty() {
        return MoveOrdersReport::default();
    }

    let layout = world.layout();
    let plan = FormationPlanner::plan_move(
        FormationKind::Grid,
        &unit_ids,
        target,
        world,
        unit_catalog,
        layout,
    );

    let mut report = MoveOrdersReport::default();
    for assignment in plan.assignments {
        let order = UnitOrder::MoveTo {
            target: assignment.target,
        };
        match issue_unit_order(
            world,
            unit_catalog,
            doodad_catalog,
            nav_config,
            assignment.unit_id,
            order,
        ) {
            Ok(()) => {
                report.issued += 1;
                report.unit_traces.push(MoveOrderUnitTrace {
                    unit_id: assignment.unit_id,
                    order,
                    error: None,
                });
            }
            Err(error) => {
                report.failed += 1;
                report.unit_traces.push(MoveOrderUnitTrace {
                    unit_id: assignment.unit_id,
                    order,
                    error: Some(error),
                });
                log_move_order_failure(assignment.unit_id, error);
            }
        }
    }
    report
}

/// Issue `Idle` orders for every selected unit (Stop / Hold placeholders).
pub fn issue_idle_orders_to_selection(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    selection: &SelectedUnits,
) -> MoveOrdersReport {
    let unit_ids: Vec<_> = selection.iter().collect();
    if unit_ids.is_empty() {
        return MoveOrdersReport::default();
    }

    let mut report = MoveOrdersReport::default();
    for unit_id in unit_ids {
        let order = UnitOrder::Idle;
        match issue_unit_order(
            world,
            unit_catalog,
            doodad_catalog,
            nav_config,
            unit_id,
            order,
        ) {
            Ok(()) => {
                report.issued += 1;
                report.unit_traces.push(MoveOrderUnitTrace {
                    unit_id,
                    order,
                    error: None,
                });
            }
            Err(error) => {
                report.failed += 1;
                report.unit_traces.push(MoveOrderUnitTrace {
                    unit_id,
                    order,
                    error: Some(error),
                });
            }
        }
    }
    report
}

pub fn log_move_order_failure(unit_id: crate::world::UnitId, error: UnitOrderError) {
    match error {
        UnitOrderError::NoPath => {
            warn!("move order for unit {} failed: no path", unit_id.raw());
        }
        UnitOrderError::PathGoalBlocked | UnitOrderError::PathStartBlocked => {
            warn!("move order for unit {} failed: blocked", unit_id.raw());
        }
        UnitOrderError::PathTerrainUnavailable => {
            warn!(
                "move order for unit {} failed: terrain unavailable",
                unit_id.raw()
            );
        }
        UnitOrderError::UnitNotFound => {}
        UnitOrderError::DefinitionNotFound => {
            warn!(
                "move order for unit {} failed: missing definition",
                unit_id.raw()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, resolve_all_pending_unit_orders, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        Heightfield, LocalPosition, UnitDefinitionId, UnitSource, UnitState, WorldPosition,
    };

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

    fn moving_target(unit_id: crate::world::UnitId, world: &WorldData) -> WorldPosition {
        match world.get_unit(unit_id).unwrap().state {
            UnitState::Moving { target, .. } => target,
            _ => panic!("expected moving"),
        }
    }

    #[test]
    fn right_click_issues_move_to_all_selected_units() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let mut world = flat_world();

        let a = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let b = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(8.0, 8.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let mut selection = SelectedUnits::default();
        selection.replace_with([a, b]);

        let target = pos(40.0, 40.0);
        let report = issue_move_orders_to_selection(
            &mut world,
            &selection,
            &catalog,
            &doodad_catalog,
            &nav_config,
            target,
        );
        resolve_all_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav_config);

        assert_eq!(report.issued, 2);
        assert_eq!(report.failed, 0);
        assert!(matches!(
            world.get_unit(a).unwrap().state,
            UnitState::Moving { .. }
        ));
        assert!(matches!(
            world.get_unit(b).unwrap().state,
            UnitState::Moving { .. }
        ));
    }

    #[test]
    fn group_move_spreads_targets_instead_of_clumping() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let mut world = flat_world();

        let a = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let b = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(8.0, 8.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let mut selection = SelectedUnits::default();
        selection.replace_with([a, b]);
        let click = pos(40.0, 40.0);
        issue_move_orders_to_selection(
            &mut world,
            &selection,
            &catalog,
            &doodad_catalog,
            &nav_config,
            click,
        );
        resolve_all_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav_config);

        let target_a = moving_target(a, &world);
        let target_b = moving_target(b, &world);
        assert_ne!(target_a, target_b);
        assert_ne!(target_a, click);
        assert_ne!(target_b, click);
    }

    #[test]
    fn each_unit_receives_independent_path_state() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let mut world = flat_world();

        let a = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let b = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(8.0, 8.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let mut selection = SelectedUnits::default();
        selection.replace_with([a, b]);
        issue_move_orders_to_selection(
            &mut world,
            &selection,
            &catalog,
            &doodad_catalog,
            &nav_config,
            pos(40.0, 40.0),
        );
        resolve_all_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav_config);

        let UnitState::Moving { path: ref path_a, .. } = world.get_unit(a).unwrap().state else {
            panic!("expected moving");
        };
        let UnitState::Moving { path: ref path_b, .. } = world.get_unit(b).unwrap().state else {
            panic!("expected moving");
        };
        assert_ne!(path_a, path_b);
    }

    #[test]
    fn selection_logic_does_not_mutate_world_data_directly() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let before = world.get_unit(unit_id).unwrap().clone();
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        selection.toggle(unit_id);
        selection.clear();
        let after = world.get_unit(unit_id).unwrap();

        assert_eq!(before, *after);
    }
}
