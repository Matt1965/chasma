//! Hauling task assignment (EP7).

use crate::world::combat::AttackTargetingPolicy;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::task::{
    TaskError, TaskEvent, TaskId, TaskPriority, TaskRecord, TaskState, TaskTarget, TaskType,
};
use crate::world::{
    DoodadCatalog, NavigationConfig, UnitCatalog, UnitId, UnitOrder, WeaponCatalog, WorldData,
    issue_unit_order,
};

use super::execute::reserve_hauling_request;
use super::id::HaulingRequestId;
use super::types::{HaulExecutionPhase, HaulingRequestStatus};

/// Assign a worker to execute one hauling request (EP7).
///
/// Player/UI callers typically pass [`TaskPriority::PlayerAssigned`]. Autonomous marketplace (SA7)
/// maps request priority into High/Normal/Low.
pub fn assign_hauling_task(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
    request_id: HaulingRequestId,
    simulation_tick: u64,
) -> Result<(TaskId, Vec<TaskEvent>), TaskError> {
    assign_hauling_task_with_priority(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        inventory_ctx,
        unit_id,
        request_id,
        TaskPriority::PlayerAssigned,
        simulation_tick,
    )
}

pub fn assign_hauling_task_with_priority(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
    request_id: HaulingRequestId,
    priority: TaskPriority,
    simulation_tick: u64,
) -> Result<(TaskId, Vec<TaskEvent>), TaskError> {
    if world.get_unit(unit_id).is_none() {
        return Err(TaskError::UnitNotEligible(unit_id));
    }
    if world.task_store().unit_task_id(unit_id).is_some() {
        return Err(TaskError::TaskAlreadyAssigned(TaskId::new(0)));
    }

    let (owning_building_id, batch) = {
        let request = world
            .hauling_request_store()
            .get(request_id)
            .ok_or(TaskError::TaskInvalidated(TaskId::new(0)))?;
        if !request.status.is_open() {
            return Err(TaskError::TaskInvalidated(TaskId::new(0)));
        }
        if let Some(existing) = request.assigned_unit_id {
            if existing != unit_id {
                return Err(TaskError::TaskAlreadyAssigned(TaskId::new(0)));
            }
        }
        (
            request.owning_building_id,
            request.remaining_quantity.min(1).max(1),
        )
    };
    reserve_hauling_request(world, request_id, batch, inventory_ctx)
        .map_err(|_| TaskError::TaskInvalidated(TaskId::new(0)))?;

    let task_id = world.task_store_mut().allocate_task_id();
    let mut record = TaskRecord::new(
        task_id,
        TaskType::Haul,
        TaskTarget::HaulRequest {
            request_id,
            owning_building_id,
        },
        priority,
        simulation_tick,
    );
    record.state = TaskState::Assigned;
    record.assigned_unit_id = Some(unit_id);
    world.task_store_mut().insert_task(record)?;

    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.assigned_unit_id = Some(unit_id);
        request.assigned_task_id = Some(task_id);
        request.status = HaulingRequestStatus::Assigned;
        request.execution_phase = HaulExecutionPhase::TravelingToSource;
    }

    let target = world
        .get_unit(unit_id)
        .map(|unit| unit.placement.position)
        .unwrap_or_else(|| {
            crate::world::WorldPosition::new(
                crate::world::ChunkCoord::new(0, 0),
                crate::world::LocalPosition::new(bevy::prelude::Vec3::ZERO),
            )
        });
    issue_unit_order(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        unit_id,
        UnitOrder::Work {
            task_id,
            target,
        },
        AttackTargetingPolicy::default(),
    )
    .map_err(|_| TaskError::UnitNotEligible(unit_id))?;
    world.task_store_mut().assign_unit(task_id, unit_id)?;

    Ok((task_id, Vec::new()))
}
