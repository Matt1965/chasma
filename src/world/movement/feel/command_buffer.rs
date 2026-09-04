//! Deferred MoveTo resolution (ADR-037 U12).

use bevy::prelude::*;

use crate::world::unit::{UnitOrder, UnitOrderError};
use crate::world::{
    BuildingCatalog, DoodadCatalog, FootprintCatalog, NavigationConfig, NavigationError,
    PassabilityCatalogs, SpaceId, UnitCatalog, UnitId, UnitState, WorldData, WorldPosition,
    find_path_with_spaces, resolve_move_goal_space, resolve_navigation_start_space,
    waypoint_space_ids,
};

/// One deferred order awaiting path resolution.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct PendingUnitOrder {
    pub unit_id: UnitId,
    pub order: UnitOrder,
}

/// Paths resolved per simulation tick (spreads large group-move cost).
pub const PATH_RESOLVE_BUDGET_PER_TICK: u32 = 16;

/// Lightweight queue so paths are committed before the first movement step.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct UnitCommandBuffer {
    pending: Vec<PendingUnitOrder>,
}

impl UnitCommandBuffer {
    pub fn enqueue(&mut self, unit_id: UnitId, order: UnitOrder) {
        if let Some(existing) = self
            .pending
            .iter_mut()
            .find(|entry| entry.unit_id == unit_id)
        {
            existing.order = order;
            return;
        }
        self.pending.push(PendingUnitOrder { unit_id, order });
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn pending_for(&self, unit_id: UnitId) -> Option<&PendingUnitOrder> {
        self.pending.iter().find(|entry| entry.unit_id == unit_id)
    }

    pub fn clear_pending(&mut self, unit_id: UnitId) {
        self.pending.retain(|entry| entry.unit_id != unit_id);
    }

    pub fn take_pending_sorted(&mut self) -> Vec<PendingUnitOrder> {
        self.pending.sort_by_key(|entry| entry.unit_id);
        std::mem::take(&mut self.pending)
    }

    /// Remove up to `budget` pending orders in deterministic [`UnitId`] order.
    pub fn drain_sorted_budget(&mut self, budget: u32) -> Vec<PendingUnitOrder> {
        if budget == 0 || self.pending.is_empty() {
            return Vec::new();
        }
        self.pending.sort_by_key(|entry| entry.unit_id);
        let take = budget.min(self.pending.len() as u32) as usize;
        self.pending.drain(..take).collect()
    }
}

/// Outcome of resolving the unit command buffer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandBufferResolveReport {
    pub resolved: u32,
    pub failed: u32,
    pub failures: Vec<(UnitId, UnitOrderError)>,
    pub successes: Vec<CommandResolveSuccess>,
}

/// Per-unit path resolution success metadata (observability only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandResolveSuccess {
    pub unit_id: UnitId,
    pub target: WorldPosition,
    pub path_waypoint_count: u32,
}

pub fn start_unit_move_to(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    target: WorldPosition,
) -> Result<(), UnitOrderError> {
    let record = world
        .get_unit(unit_id)
        .ok_or(UnitOrderError::UnitNotFound)?;
    let definition_id = record.definition_id.clone();
    let start = record.placement.position;
    let unit_space_before = record.current_space_id;
    let start_space = resolve_navigation_start_space(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        start,
        unit_space_before,
    );
    if start_space != unit_space_before {
        let _ = world.set_unit_current_space(unit_id, start_space);
    }
    let definition = unit_catalog
        .get(&definition_id)
        .ok_or(UnitOrderError::DefinitionNotFound)?;
    let unit_ownership = world.get_unit(unit_id).map(|record| record.ownership());
    let (goal_space, grounded_goal) = resolve_move_goal_space(world, start_space, target);
    #[cfg(feature = "dev")]
    let post_exit_trace_snapshot = if world.post_exit_jitter_trace().is_active_for(unit_id) {
        world
            .get_unit(unit_id)
            .and_then(|record| match &record.state {
                UnitState::Moving {
                    target: old_goal,
                    path: old_path,
                    waypoint_index,
                } => Some((
                    *waypoint_index,
                    *old_goal,
                    old_path.clone(),
                    record.current_space_id,
                    record.placement.position,
                )),
                _ => None,
            })
    } else {
        None
    };
    #[cfg(feature = "dev")]
    let exit_click_trace_active = world
        .interior_exit_click_trace()
        .is_active_for_target(unit_id, target);
    #[cfg(feature = "dev")]
    if exit_click_trace_active {
        crate::world::interior_exit_click_trace::record_start_unit_move_to(
            world,
            unit_id,
            target,
            unit_space_before,
            start_space,
            goal_space,
            grounded_goal,
        );
    }
    let path_result = find_path_with_spaces(
        world,
        catalogs,
        nav_config,
        definition.collision_radius_meters,
        definition.max_slope_degrees,
        start,
        grounded_goal,
        start_space,
        goal_space,
        unit_ownership,
    );
    #[cfg(feature = "dev")]
    if world.entrance_traversal_trace().is_active_for(unit_id) {
        crate::world::record_entrance_pathfinding_probe(
            world,
            unit_id,
            catalogs,
            nav_config,
            definition.collision_radius_meters,
            definition.max_slope_degrees,
            start,
            grounded_goal,
            start_space,
            goal_space,
            unit_ownership,
            path_result.as_ref().map_err(|error| *error),
        );
    }
    #[cfg(feature = "dev")]
    if world.inside_move_trace().is_active_for(unit_id) {
        let result_label = match &path_result {
            Ok(path) => "success",
            Err(error) => navigation_error_label(error),
        };
        let waypoint_count = path_result.as_ref().ok().map(|path| path.len() as u32);
        crate::world::inside_move_trace::record_path_resolution(
            world,
            unit_id,
            unit_space_before,
            start_space,
            goal_space,
            result_label,
            waypoint_count,
        );
    }
    #[cfg(feature = "dev")]
    if exit_click_trace_active {
        if matches!(
            path_result.as_ref(),
            Err(crate::world::NavigationError::GoalBlocked)
        ) && !start_space.is_surface()
            && goal_space.is_surface()
        {
            crate::world::interior_exit_click_trace::record_surface_goal_passability_probe(
                world,
                unit_id,
                target,
                catalogs,
                definition.collision_radius_meters,
                definition.max_slope_degrees,
                grounded_goal,
                start_space,
                goal_space,
                unit_ownership,
            );
        }
        let result_label = match &path_result {
            Ok(path) => "success",
            Err(error) => navigation_error_label(error),
        };
        let waypoint_count = path_result.as_ref().ok().map(|path| path.len() as u32);
        let portal_waypoint = path_result
            .as_ref()
            .ok()
            .and_then(|path| path.waypoints.iter().find(|wp| wp.portal_id.is_some()));
        let portal_id = portal_waypoint.and_then(|wp| wp.portal_id.map(|id| id.raw()));
        let first_wp = path_result
            .as_ref()
            .ok()
            .and_then(|path| path.waypoints.first().map(|wp| wp.position));
        let final_wp = path_result
            .as_ref()
            .ok()
            .and_then(|path| path.waypoints.last().map(|wp| wp.position));
        crate::world::interior_exit_click_trace::record_path_result(
            world,
            unit_id,
            target,
            result_label,
            waypoint_count,
            portal_waypoint.is_some(),
            portal_id,
            first_wp,
            final_wp,
        );
        if path_result.is_err() {
            crate::world::interior_exit_click_trace::record_post_resolution_state(
                world,
                unit_id,
                target,
                "Idle",
                false,
                None,
                unit_space_before,
            );
        }
    }
    #[cfg(feature = "dev")]
    if let Some((old_waypoint_index, old_goal, old_path, old_space, old_position)) =
        post_exit_trace_snapshot
    {
        crate::world::unit::post_exit_jitter_trace::record_new_order_during_session(
            world,
            unit_id,
            old_waypoint_index,
            old_waypoint_index,
            old_position,
            old_goal,
            old_space,
            &old_path,
            grounded_goal,
            start_space,
            path_result.as_ref().ok(),
        );
    }
    let path = path_result.map_err(map_navigation_error)?;
    if path.is_empty() {
        #[cfg(feature = "dev")]
        if exit_click_trace_active {
            crate::world::interior_exit_click_trace::record_path_result(
                world,
                unit_id,
                target,
                "NoPath",
                Some(0),
                false,
                None,
                None,
                None,
            );
            crate::world::interior_exit_click_trace::record_post_resolution_state(
                world,
                unit_id,
                target,
                "Idle",
                false,
                Some(0),
                unit_space_before,
            );
        }
        return Err(UnitOrderError::NoPath);
    }
    world.movement_authority_trace_mut().record_command(
        crate::world::MovementCommandAuthorityRecord {
            sequence: 0,
            unit_id,
            unit_space_before,
            click_target: target,
            start_space,
            goal_space,
            grounded_goal,
            waypoint_spaces: waypoint_space_ids(&path.waypoints),
        },
    );
    world
        .portal_transition_state_mut(unit_id)
        .suppress_return_space = None;
    world
        .set_unit_state(
            unit_id,
            UnitState::Moving {
                target,
                path,
                waypoint_index: 0,
            },
        )
        .map_err(|_| UnitOrderError::UnitNotFound)?;
    #[cfg(feature = "dev")]
    if exit_click_trace_active {
        let record = world
            .get_unit(unit_id)
            .ok_or(UnitOrderError::UnitNotFound)?;
        let (state_label, path_stored, waypoint_count) = match &record.state {
            UnitState::Moving { path, .. } => ("Moving", true, path.len() as u32),
            UnitState::Idle => ("Idle", false, 0),
            _ => ("other", false, 0),
        };
        crate::world::interior_exit_click_trace::record_post_resolution_state(
            world,
            unit_id,
            target,
            state_label,
            path_stored,
            if path_stored {
                Some(waypoint_count)
            } else {
                None
            },
            record.current_space_id,
        );
    }
    Ok(())
}

pub(crate) fn resolve_one(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    order: UnitOrder,
) -> Result<(), UnitOrderError> {
    if world.get_unit(unit_id).is_none() {
        return Err(UnitOrderError::UnitNotFound);
    }
    if !crate::world::unit::unit_can_execute_actions(world, unit_id) {
        return Err(UnitOrderError::UnitNotFound);
    }

    match order {
        UnitOrder::Idle => world
            .set_unit_state(unit_id, UnitState::Idle)
            .map_err(|_| UnitOrderError::UnitNotFound),
        UnitOrder::MoveTo { target } => {
            start_unit_move_to(world, unit_catalog, catalogs, nav_config, unit_id, target)
        }
        UnitOrder::Work { target, task_id } => {
            if world.get_unit(unit_id).is_some_and(|unit| {
                matches!(
                    unit.state,
                    UnitState::Working {
                        task_id: working_task_id
                    } if working_task_id == task_id
                )
            }) {
                return Ok(());
            }
            let unit = world
                .get_unit(unit_id)
                .ok_or(UnitOrderError::UnitNotFound)?;
            let layout = world.layout();
            let unit_global = unit.placement.position.to_global(layout);
            let work_global = target.to_global(layout);
            let dx = unit_global.x - work_global.x;
            let dz = unit_global.z - work_global.z;
            if (dx * dx + dz * dz).sqrt() <= crate::world::INTERACTION_WORK_RANGE_METERS {
                return world
                    .set_unit_state(unit_id, UnitState::Working { task_id })
                    .map_err(|_| UnitOrderError::UnitNotFound);
            }
            start_unit_move_to(world, unit_catalog, catalogs, nav_config, unit_id, target)
        }
        UnitOrder::Attack { .. } | UnitOrder::AttackMove { .. } => {
            Err(UnitOrderError::AttackerNotFound)
        }
    }
}

fn map_navigation_error(error: NavigationError) -> UnitOrderError {
    match error {
        NavigationError::StartBlocked => UnitOrderError::PathStartBlocked,
        NavigationError::GoalBlocked => UnitOrderError::PathGoalBlocked,
        NavigationError::NoPath => UnitOrderError::NoPath,
        NavigationError::TerrainUnavailable => UnitOrderError::PathTerrainUnavailable,
    }
}

#[cfg(feature = "dev")]
fn navigation_error_label(error: &NavigationError) -> &'static str {
    match error {
        NavigationError::StartBlocked => "StartBlocked",
        NavigationError::GoalBlocked => "GoalBlocked",
        NavigationError::NoPath => "NoPath",
        NavigationError::TerrainUnavailable => "TerrainUnavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
        FootprintCatalog, Heightfield, LocalPosition, PassabilityCatalogs, UnitDefinitionId,
        UnitSource, create_unit,
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

    #[test]
    fn large_batch_spreads_path_resolution_across_ticks() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav = NavigationConfig::default();
        let mut world = flat_world();

        for index in 0..40 {
            create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("wolf"),
                pos(10.0 + index as f32 * 0.5, 10.0),
                UnitSource::Authored,
            )
            .unwrap();
        }

        for unit_id in world.sorted_unit_ids() {
            world.command_buffer_mut().enqueue(
                unit_id,
                UnitOrder::MoveTo {
                    target: pos(80.0, 40.0),
                },
            );
        }

        let first = crate::world::resolve_pending_unit_orders(
            &mut world,
            &catalog,
            PassabilityCatalogs {
                doodad: &doodad_catalog,
                building: &BuildingCatalog::default(),
                footprint: &FootprintCatalog::default(),
            },
            &nav,
        );
        assert_eq!(first.resolved, PATH_RESOLVE_BUDGET_PER_TICK);
        assert!(!world.command_buffer().is_empty());

        let mut total = first.resolved;
        while !world.command_buffer().is_empty() {
            let batch = crate::world::resolve_pending_unit_orders(
                &mut world,
                &catalog,
                PassabilityCatalogs {
                    doodad: &doodad_catalog,
                    building: &BuildingCatalog::default(),
                    footprint: &FootprintCatalog::default(),
                },
                &nav,
            );
            total += batch.resolved;
        }
        assert_eq!(total, 40);
    }

    #[test]
    fn buffer_resolves_move_in_one_tick_without_movement_before_path() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav = NavigationConfig::default();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        world.command_buffer_mut().enqueue(
            unit_id,
            UnitOrder::MoveTo {
                target: pos(80.0, 40.0),
            },
        );
        assert_eq!(world.get_unit(unit_id).unwrap().state, UnitState::Idle);
        assert!(world.command_buffer().pending_for(unit_id).is_some());

        let report = crate::world::resolve_pending_unit_orders(
            &mut world,
            &catalog,
            PassabilityCatalogs {
                doodad: &doodad_catalog,
                building: &BuildingCatalog::default(),
                footprint: &FootprintCatalog::default(),
            },
            &nav,
        );
        assert_eq!(report.resolved, 1);
        assert!(matches!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Moving { .. }
        ));
    }
}
