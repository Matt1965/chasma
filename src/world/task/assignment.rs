use bevy::prelude::*;

use super::eligibility::{
    building_is_constructible, unit_can_perform_task, unit_may_work_on_building,
};
use super::error::TaskError;
use super::events::TaskEvent;
use super::id::TaskId;
use super::record::TaskRecord;
use super::types::{TaskCancelReason, TaskPriority, TaskState, TaskTarget, TaskType};
use crate::world::combat::AttackTargetingPolicy;
use crate::world::{
    BuildingCatalog, BuildingId, DoodadCatalog, NavigationConfig, UnitCatalog, UnitId, UnitOrder,
    WorldData, issue_unit_order,
};
use crate::world::{
    BuildingInteractionProfile, BuildingInteractionProfileCatalog, InteractionPointDefinition,
    interaction_point_world_position,
};

pub fn assign_construct_building_task(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &crate::world::WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    building_id: BuildingId,
    simulation_tick: u64,
) -> Result<(TaskId, Vec<TaskEvent>), TaskError> {
    claim_building_task(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        building_catalog,
        interaction_catalog,
        nav_config,
        unit_id,
        building_id,
        TaskType::ConstructBuilding,
        TaskPriority::PlayerAssigned,
        simulation_tick,
    )
}

pub fn assign_operate_workstation_task(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &crate::world::WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    building_id: BuildingId,
    simulation_tick: u64,
) -> Result<(TaskId, Vec<TaskEvent>), TaskError> {
    claim_building_task(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        building_catalog,
        interaction_catalog,
        nav_config,
        unit_id,
        building_id,
        TaskType::OperateWorkstation,
        TaskPriority::PlayerAssigned,
        simulation_tick,
    )
}

/// Claim an existing or ensured building task (player or autonomous marketplace / SA7).
pub fn claim_building_task(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &crate::world::WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    building_id: BuildingId,
    task_type: TaskType,
    priority: TaskPriority,
    simulation_tick: u64,
) -> Result<(TaskId, Vec<TaskEvent>), TaskError> {
    let mut events = Vec::new();
    let building = world
        .get_building(building_id)
        .cloned()
        .ok_or(TaskError::BuildingNotFound(building_id))?;
    let unit = world
        .get_unit(unit_id)
        .ok_or(TaskError::UnitNotEligible(unit_id))?;
    if !unit_can_perform_task(unit_catalog, world, unit_id, task_type) {
        return Err(TaskError::UnitNotEligible(unit_id));
    }
    if !unit_may_work_on_building(&building, unit.ownership()) {
        return Err(TaskError::Unauthorized {
            unit_id,
            building_id,
        });
    }
    let definition = building_catalog
        .get(&building.definition_id)
        .ok_or(TaskError::DefinitionNotFound)?;
    let profile = interaction_catalog
        .profile_for_definition(definition)
        .ok_or(TaskError::InteractionPointMissing {
            building_id,
            point_key: "profile".into(),
        })?;
    match task_type {
        TaskType::ConstructBuilding if !building_is_constructible(&building) => {
            return Err(TaskError::BuildingNotConstructible(building_id));
        }
        TaskType::OperateWorkstation
            if !super::eligibility::building_accepts_workstation_use(&building) =>
        {
            return Err(TaskError::BuildingNotOperational(building_id));
        }
        TaskType::Haul => {
            return Err(TaskError::TaskInvalidated(TaskId::new(0)));
        }
        _ => {}
    }

    let task_id = ensure_building_task(world, building_id, task_type, priority, simulation_tick)?;
    let point_key =
        reserve_nearest_point(world, &building, profile, task_type, unit_id, &mut events)?;
    world
        .task_store_mut()
        .assign_unit(task_id, unit_id)
        .map_err(|_| TaskError::TaskAlreadyAssigned(task_id))?;
    if let Some(task) = world.task_store_mut().get_mut(task_id) {
        task.reserved_point_key = Some(point_key.clone());
    }
    events.push(TaskEvent::TaskAssigned { task_id, unit_id });

    let point = profile
        .points
        .iter()
        .find(|point| point.key == point_key)
        .ok_or(TaskError::InteractionPointMissing {
            building_id,
            point_key: point_key.clone(),
        })?;
    let target = interaction_point_world_position(&building, world.layout(), point);
    if issue_unit_order(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        unit_id,
        UnitOrder::Work { task_id, target },
        AttackTargetingPolicy::default(),
    )
    .is_err()
    {
        cancel_unit_task(
            world,
            unit_id,
            TaskCancelReason::PathUnavailable,
            &mut events,
        );
        return Err(TaskError::PathUnavailable(unit_id));
    }
    Ok((task_id, events))
}

pub fn ensure_building_task(
    world: &mut WorldData,
    building_id: BuildingId,
    task_type: TaskType,
    priority: TaskPriority,
    simulation_tick: u64,
) -> Result<TaskId, TaskError> {
    for task_id in world.task_store().building_task_ids(building_id).to_vec() {
        if let Some(task) = world.task_store().get(task_id) {
            if task.task_type == task_type
                && matches!(
                    task.state,
                    TaskState::Available | TaskState::Assigned | TaskState::InProgress
                )
            {
                return Ok(task_id);
            }
        }
    }
    let task_id = world.task_store_mut().allocate_task_id();
    let record = TaskRecord::new(
        task_id,
        task_type,
        TaskTarget::Building(building_id),
        priority,
        simulation_tick,
    );
    world.task_store_mut().insert_task(record)?;
    Ok(task_id)
}

fn reserve_nearest_point(
    world: &mut WorldData,
    building: &crate::world::BuildingRecord,
    profile: &BuildingInteractionProfile,
    task_type: TaskType,
    unit_id: UnitId,
    events: &mut Vec<TaskEvent>,
) -> Result<String, TaskError> {
    let layout = world.layout();
    let unit_pos = world
        .get_unit(unit_id)
        .map(|record| record.placement.position.to_global(layout))
        .ok_or(TaskError::UnitNotEligible(unit_id))?;
    let mut candidates: Vec<_> = profile
        .points
        .iter()
        .filter(|point| point.task_type == task_type && point.enabled_for(building.lifecycle_state))
        .collect();
    candidates.sort_by_key(|point| {
        let pos = interaction_point_world_position(building, layout, point).to_global(layout);
        let dx = pos.x - unit_pos.x;
        let dz = pos.z - unit_pos.z;
        (dx * dx + dz * dz).to_bits()
    });
    for point in candidates {
        if world
            .task_store()
            .reservation_for_point(building.id, point.key)
            .is_some()
        {
            continue;
        }
        world
            .task_store_mut()
            .reserve_point(building.id, point.key, unit_id)?;
        events.push(TaskEvent::InteractionPointReserved {
            building_id: building.id,
            point_key: point.key.to_string(),
            unit_id,
        });
        return Ok(point.key.to_string());
    }
    Err(TaskError::InteractionPointOccupied {
        building_id: building.id,
        point_key: "all".into(),
    })
}

pub fn cancel_unit_task(
    world: &mut WorldData,
    unit_id: UnitId,
    reason: TaskCancelReason,
    events: &mut Vec<TaskEvent>,
) {
    let Some(task_id) = world.task_store().unit_task_id(unit_id) else {
        return;
    };
    if let Some(task) = world.task_store().get(task_id).cloned() {
        if let Some(point_key) = task.reserved_point_key.as_deref() {
            world.task_store_mut().release_reservation(
                task.target_building_id(),
                point_key,
                unit_id,
            );
            events.push(TaskEvent::ReservationReleased {
                building_id: task.target_building_id(),
                point_key: point_key.to_string(),
                unit_id,
            });
        }
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            if !matches!(task.state, TaskState::Completed | TaskState::Canceled) {
                task.state = TaskState::Canceled;
            }
            task.assigned_unit_id = None;
            task.reserved_point_key = None;
        }
        world.task_store_mut().clear_unit_assignment(unit_id);
        events.push(TaskEvent::TaskCanceled { task_id, reason });
    }
    let _ = world.set_unit_state(unit_id, crate::world::UnitState::Idle);
}

/// Release a worker back to Idle and return the task to Available (SA7 preemption).
///
/// Unlike [`cancel_unit_task`], the marketplace listing survives for other workers.
pub fn release_unit_task_to_marketplace(
    world: &mut WorldData,
    unit_id: UnitId,
    events: &mut Vec<TaskEvent>,
) {
    let Some(task_id) = world.task_store().unit_task_id(unit_id) else {
        return;
    };
    if let Some(task) = world.task_store().get(task_id).cloned() {
        if let Some(point_key) = task.reserved_point_key.as_deref() {
            world.task_store_mut().release_reservation(
                task.target_building_id(),
                point_key,
                unit_id,
            );
            events.push(TaskEvent::ReservationReleased {
                building_id: task.target_building_id(),
                point_key: point_key.to_string(),
                unit_id,
            });
        }
    }
    // clear_unit_assignment returns Assigned/InProgress → Available.
    world.task_store_mut().clear_unit_assignment(unit_id);
    let _ = world.set_unit_state(unit_id, crate::world::UnitState::Idle);
    let _ = task_id;
}
