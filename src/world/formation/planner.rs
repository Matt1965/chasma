//! Formation move planning (ADR-035 U10).

use bevy::prelude::Vec3;

use crate::world::{ChunkLayout, UnitCatalog, UnitId, WorldData, WorldPosition};

use super::distribution::formation_offsets;
use super::layout::FormationKind;
use super::offsets::{FormationOffset, formation_jitter, unit_spacing_meters};

/// Per-unit move target derived from a group move command.
#[derive(Debug, Clone, PartialEq)]
pub struct FormationAssignment {
    pub unit_id: UnitId,
    pub target: WorldPosition,
}

/// Result of [`FormationPlanner::plan_move`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormationMovePlan {
    pub assignments: Vec<FormationAssignment>,
}

/// Distributes group move targets around a click point without pathfinding.
pub struct FormationPlanner;

impl FormationPlanner {
    /// Compute per-unit grounded targets for a group move to `target`.
    ///
    /// `unit_ids` may be unsorted; assignments are ordered by [`UnitId`].
    pub fn plan_move(
        kind: FormationKind,
        unit_ids: &[UnitId],
        target: WorldPosition,
        world: &WorldData,
        catalog: &UnitCatalog,
        layout: ChunkLayout,
    ) -> FormationMovePlan {
        if unit_ids.is_empty() {
            return FormationMovePlan::default();
        }

        let mut sorted: Vec<UnitId> = unit_ids.to_vec();
        sorted.sort_unstable();

        if sorted.len() == 1 {
            let unit_id = sorted[0];
            let mut batch = std::collections::HashMap::new();
            let validated = super::destination_validation::resolve_move_destination(
                unit_id, target, world, catalog, layout, &batch,
            );
            batch.insert(unit_id, validated);
            return FormationMovePlan {
                assignments: vec![FormationAssignment {
                    unit_id,
                    target: validated,
                }],
            };
        }

        let spacing = group_spacing_meters(&sorted, world, catalog);
        let mut slot_offsets = formation_offsets(kind, sorted.len(), spacing);
        apply_jitter(&mut slot_offsets, &sorted, target, layout);

        let center = target.to_global(layout);
        let mut batch_resolved = std::collections::HashMap::new();
        let assignments = sorted
            .into_iter()
            .zip(slot_offsets)
            .map(|(unit_id, offset)| {
                let global = Vec3::new(center.x + offset.xz.x, center.y, center.z + offset.xz.y);
                let proposed = WorldPosition::from_global(global, layout);
                let validated = super::destination_validation::resolve_move_destination(
                    unit_id,
                    proposed,
                    world,
                    catalog,
                    layout,
                    &batch_resolved,
                );
                batch_resolved.insert(unit_id, validated);
                FormationAssignment {
                    unit_id,
                    target: validated,
                }
            })
            .collect();

        FormationMovePlan { assignments }
    }
}

fn group_spacing_meters(unit_ids: &[UnitId], world: &WorldData, catalog: &UnitCatalog) -> f32 {
    unit_ids
        .iter()
        .filter_map(|unit_id| world.get_unit(*unit_id))
        .filter_map(|record| catalog.get(&record.definition_id))
        .map(|definition| unit_spacing_meters(definition.collision_radius_meters))
        .fold(0.0_f32, f32::max)
        .max(super::offsets::FORMATION_MIN_SPACING_METERS)
}

fn apply_jitter(
    offsets: &mut [FormationOffset],
    sorted_unit_ids: &[UnitId],
    target: WorldPosition,
    layout: ChunkLayout,
) {
    for (offset, &unit_id) in offsets.iter_mut().zip(sorted_unit_ids) {
        offset.xz += formation_jitter(unit_id, target, layout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, DoodadCatalog, FootprintCatalog,
        Heightfield, LocalPosition, NavigationConfig, PassabilityCatalogs, UnitDefinitionId,
        UnitSource, create_unit,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
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

    fn spawn(catalog: &UnitCatalog, world: &mut WorldData, x: f32, z: f32) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    #[test]
    fn single_unit_formation_equals_direct_move_to() {
        let catalog = UnitCatalog::default();
        let world = flat_world();
        let target = pos(40.0, 40.0);
        let unit_id = UnitId::new(1);
        let plan = FormationPlanner::plan_move(
            FormationKind::Circle,
            &[unit_id],
            target,
            &world,
            &catalog,
            layout(),
        );
        assert_eq!(plan.assignments.len(), 1);
        assert_eq!(plan.assignments[0].unit_id, unit_id);
        assert_eq!(plan.assignments[0].target, target);
    }

    #[test]
    fn multiple_units_spread_around_target() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let ids = [
            spawn(&catalog, &mut world, 4.0, 4.0),
            spawn(&catalog, &mut world, 8.0, 8.0),
            spawn(&catalog, &mut world, 12.0, 12.0),
        ];
        let target = pos(40.0, 40.0);
        let plan = FormationPlanner::plan_move(
            FormationKind::Circle,
            &ids,
            target,
            &world,
            &catalog,
            layout(),
        );
        assert_eq!(plan.assignments.len(), 3);

        let center = target.to_global(layout());
        let mut distinct = 0;
        for assignment in &plan.assignments {
            let global = assignment.target.to_global(layout());
            let delta = Vec3::new(global.x - center.x, 0.0, global.z - center.z);
            if delta.length() > 0.25 {
                distinct += 1;
            }
        }
        assert!(distinct >= 2);
    }

    #[test]
    fn deterministic_formation_output_for_same_inputs() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let ids = [
            spawn(&catalog, &mut world, 1.0, 1.0),
            spawn(&catalog, &mut world, 2.0, 2.0),
        ];
        let target = pos(30.0, 30.0);
        let a = FormationPlanner::plan_move(
            FormationKind::Circle,
            &ids,
            target,
            &world,
            &catalog,
            layout(),
        );
        let b = FormationPlanner::plan_move(
            FormationKind::Circle,
            &ids,
            target,
            &world,
            &catalog,
            layout(),
        );
        assert_eq!(a, b);
    }

    #[test]
    fn formation_does_not_depend_on_selection_order() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let a = spawn(&catalog, &mut world, 1.0, 1.0);
        let b = spawn(&catalog, &mut world, 2.0, 2.0);
        let c = spawn(&catalog, &mut world, 3.0, 3.0);
        let target = pos(50.0, 50.0);
        let forward = FormationPlanner::plan_move(
            FormationKind::Circle,
            &[a, b, c],
            target,
            &world,
            &catalog,
            layout(),
        );
        let shuffled = FormationPlanner::plan_move(
            FormationKind::Circle,
            &[c, a, b],
            target,
            &world,
            &catalog,
            layout(),
        );
        assert_eq!(forward, shuffled);
    }

    #[test]
    fn sorted_unit_id_produces_stable_assignment_order() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let ids = [
            spawn(&catalog, &mut world, 1.0, 1.0),
            spawn(&catalog, &mut world, 2.0, 2.0),
        ];
        let plan = FormationPlanner::plan_move(
            FormationKind::Circle,
            &ids,
            pos(20.0, 20.0),
            &world,
            &catalog,
            layout(),
        );
        assert!(plan.assignments[0].unit_id < plan.assignments[1].unit_id);
    }

    #[test]
    fn spacing_uses_max_collision_radius_in_group() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let wolf = spawn(&catalog, &mut world, 1.0, 1.0);
        let ids = [wolf];
        let spacing = super::group_spacing_meters(&ids, &world, &catalog);
        let wolf_radius = catalog
            .get(&UnitDefinitionId::new("wolf"))
            .unwrap()
            .collision_radius_meters;
        assert!((spacing - unit_spacing_meters(wolf_radius)).abs() < 1e-4);
    }

    #[test]
    fn single_unit_move_onto_occupant_is_projected() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let idle = spawn(&catalog, &mut world, 20.0, 20.0);
        let mover = spawn(&catalog, &mut world, 4.0, 4.0);
        let click = world.get_unit(idle).unwrap().placement.position;
        let plan = FormationPlanner::plan_move(
            FormationKind::Grid,
            &[mover],
            click,
            &world,
            &catalog,
            layout(),
        );
        assert_ne!(plan.assignments[0].target, click);
    }

    #[test]
    fn assignments_produce_reachable_targets_on_flat_terrain() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let mut world = flat_world();
        let ids = [
            spawn(&catalog, &mut world, 4.0, 4.0),
            spawn(&catalog, &mut world, 8.0, 8.0),
        ];
        let target = pos(40.0, 40.0);
        let plan = FormationPlanner::plan_move(
            FormationKind::Circle,
            &ids,
            target,
            &world,
            &catalog,
            layout(),
        );
        for assignment in plan.assignments {
            let result = crate::world::issue_unit_order(
                &mut world,
                &catalog,
                &crate::world::WeaponCatalog::default(),
                &doodad_catalog,
                &nav_config,
                assignment.unit_id,
                crate::world::UnitOrder::MoveTo {
                    target: assignment.target,
                },
                crate::world::AttackTargetingPolicy::default(),
            );
            assert!(result.is_ok());
        }
        let report = crate::world::resolve_all_pending_unit_orders(
            &mut world,
            &catalog,
            PassabilityCatalogs {
                doodad: &doodad_catalog,
                building: &BuildingCatalog::default(),
                footprint: &FootprintCatalog::default(),
            },
            &nav_config,
        );
        assert_eq!(report.resolved, 2);
    }
}
