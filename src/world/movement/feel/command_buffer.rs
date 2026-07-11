//! Deferred MoveTo resolution (ADR-037 U12).

use bevy::prelude::*;

use crate::world::unit::{UnitOrder, UnitOrderError};
use crate::world::{
    DoodadCatalog, NavigationConfig, NavigationError, UnitCatalog, UnitId, UnitState, WorldData,
    WorldPosition, find_path,
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
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    target: WorldPosition,
) -> Result<(), UnitOrderError> {
    let record = world
        .get_unit(unit_id)
        .ok_or(UnitOrderError::UnitNotFound)?;
    let definition_id = record.definition_id.clone();
    let start = record.placement.position;
    let definition = unit_catalog
        .get(&definition_id)
        .ok_or(UnitOrderError::DefinitionNotFound)?;
    let path = find_path(
        world,
        doodad_catalog,
        nav_config,
        definition.collision_radius_meters,
        definition.max_slope_degrees,
        start,
        target,
    )
    .map_err(map_navigation_error)?;
    if path.is_empty() {
        return Err(UnitOrderError::NoPath);
    }
    world
        .set_unit_state(
            unit_id,
            UnitState::Moving {
                target,
                path,
                waypoint_index: 0,
            },
        )
        .map_err(|_| UnitOrderError::UnitNotFound)
}

pub(crate) fn resolve_one(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
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
        UnitOrder::MoveTo { target } => start_unit_move_to(
            world,
            unit_catalog,
            doodad_catalog,
            nav_config,
            unit_id,
            target,
        ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitDefinitionId,
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

        let first =
            crate::world::resolve_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav);
        assert_eq!(first.resolved, PATH_RESOLVE_BUDGET_PER_TICK);
        assert!(!world.command_buffer().is_empty());

        let mut total = first.resolved;
        while !world.command_buffer().is_empty() {
            let batch = crate::world::resolve_pending_unit_orders(
                &mut world,
                &catalog,
                &doodad_catalog,
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

        let report =
            crate::world::resolve_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav);
        assert_eq!(report.resolved, 1);
        assert!(matches!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Moving { .. }
        ));
    }
}
