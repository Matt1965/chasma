//! Worker hauling task stepping (EP7).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::{InventoryCatalogCtx, max_accept_stack_quantity};
use crate::world::movement::feel::start_unit_move_to;
use crate::world::task::{TaskCancelReason, TaskState, TaskType, cancel_unit_task};
use crate::world::{
    BuildingId, BuildingInteractionProfileCatalog, INTERACTION_WORK_RANGE_METERS, NavigationConfig,
    PassabilityCatalogs, UnitCatalog, UnitId, UnitState, WorldData, WorldPosition,
    interaction_point_world_position,
};

use super::execute::{
    cancel_hauling_request, deposit_haul_cargo, pickup_haul_cargo, reserve_hauling_request,
};
use super::id::HaulingRequestId;
use super::reservation::release_request_reservations;
use super::types::{
    HaulExecutionPhase, HaulingBlockingReason, HaulingRequestStatus, HaulingReservationState,
};

/// Per-tick hauling labor report (EP7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HaulTickReport {
    pub pickups: u32,
    pub deposits: u32,
    pub completions: u32,
    pub cancellations: u32,
}

/// Step all active haul tasks (EP7).
pub fn step_haul_worker_tasks(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    unit_catalog: &UnitCatalog,
    passability: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    simulation_tick: u64,
) -> HaulTickReport {
    let mut report = HaulTickReport::default();
    let unit_ids = world.sorted_unit_ids();
    for unit_id in unit_ids {
        let Some(task_id) = world.task_store().unit_task_id(unit_id) else {
            continue;
        };
        let Some(task) = world.task_store().get(task_id).cloned() else {
            continue;
        };
        if task.task_type != TaskType::Haul {
            continue;
        }
        let request_id = task
            .hauling_request_id()
            .unwrap_or(HaulingRequestId::INVALID);
        if !request_id.is_valid() {
            recover_orphaned_haul_assignment(world, request_id);
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::Invalidated,
                &mut Vec::new(),
            );
            report.cancellations += 1;
            continue;
        }
        step_one_haul(
            world,
            building_catalog,
            interaction_catalog,
            inventory_ctx,
            unit_catalog,
            passability,
            nav_config,
            simulation_tick,
            unit_id,
            task_id,
            request_id,
            &mut report,
        );
    }
    report
}

fn step_one_haul(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    unit_catalog: &UnitCatalog,
    passability: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    simulation_tick: u64,
    unit_id: UnitId,
    task_id: crate::world::TaskId,
    request_id: HaulingRequestId,
    report: &mut HaulTickReport,
) {
    let Some(request_snapshot) = world.hauling_request_store().get(request_id).cloned() else {
        recover_orphaned_haul_assignment(world, request_id);
        cancel_unit_task(
            world,
            unit_id,
            TaskCancelReason::Invalidated,
            &mut Vec::new(),
        );
        report.cancellations += 1;
        return;
    };
    if matches!(
        request_snapshot.status,
        HaulingRequestStatus::Completed | HaulingRequestStatus::Cancelled
    ) {
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            task.state = TaskState::Completed;
        }
        report.completions += 1;
        return;
    }

    let worker_inventory = match world.get_unit(unit_id).and_then(|unit| unit.inventory_id) {
        Some(id) => id,
        None => {
            block_request(
                world,
                request_id,
                &request_snapshot.item_id,
                HaulingBlockingReason::WorkerUnavailable,
                simulation_tick,
            );
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::Invalidated,
                &mut Vec::new(),
            );
            report.cancellations += 1;
            return;
        }
    };

    let layout = world.layout();
    let unit_pos = world.get_unit(unit_id).unwrap().placement.position;
    let source_pos = inventory_building_position(
        world,
        building_catalog,
        interaction_catalog,
        request_snapshot.source_inventory_id,
    )
    .unwrap_or(unit_pos);
    let dest_pos = inventory_building_position(
        world,
        building_catalog,
        interaction_catalog,
        request_snapshot.destination_inventory_id,
    )
    .unwrap_or(unit_pos);

    let phase = request_snapshot.execution_phase;
    match phase {
        HaulExecutionPhase::Pending | HaulExecutionPhase::TravelingToSource => {
            if !within_range(unit_pos, source_pos, layout) {
                ensure_traveling_toward(
                    world,
                    unit_catalog,
                    passability,
                    nav_config,
                    unit_id,
                    source_pos,
                );
                if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
                    request.execution_phase = HaulExecutionPhase::TravelingToSource;
                    request.status = HaulingRequestStatus::InProgress;
                }
                return;
            }
            let haul_qty = haul_batch_quantity(world, request_id, worker_inventory, inventory_ctx);
            if haul_qty == 0 {
                block_request(
                    world,
                    request_id,
                    &request_snapshot.item_id,
                    HaulingBlockingReason::NoAvailableItems,
                    simulation_tick,
                );
                cancel_unit_task(
                    world,
                    unit_id,
                    TaskCancelReason::Invalidated,
                    &mut Vec::new(),
                );
                report.cancellations += 1;
                return;
            }
            if request_snapshot.reservation_state == HaulingReservationState::None {
                if reserve_hauling_request(world, request_id, haul_qty, inventory_ctx).is_err() {
                    block_request(
                        world,
                        request_id,
                        &request_snapshot.item_id,
                        HaulingBlockingReason::ReservationFailed,
                        simulation_tick,
                    );
                    cancel_unit_task(
                        world,
                        unit_id,
                        TaskCancelReason::Invalidated,
                        &mut Vec::new(),
                    );
                    report.cancellations += 1;
                    return;
                }
            }
            if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
                request.execution_phase = HaulExecutionPhase::PickingUp;
            }
            match pickup_haul_cargo(world, request_id, worker_inventory, haul_qty, inventory_ctx) {
                Ok(moved) => {
                    report.pickups += moved;
                }
                Err(reason) => {
                    block_request(
                        world,
                        request_id,
                        &request_snapshot.item_id,
                        reason,
                        simulation_tick,
                    );
                    cancel_unit_task(
                        world,
                        unit_id,
                        TaskCancelReason::Invalidated,
                        &mut Vec::new(),
                    );
                    report.cancellations += 1;
                    return;
                }
            }
        }
        HaulExecutionPhase::PickingUp | HaulExecutionPhase::TravelingToDestination => {
            if !within_range(unit_pos, dest_pos, layout) {
                ensure_traveling_toward(
                    world,
                    unit_catalog,
                    passability,
                    nav_config,
                    unit_id,
                    dest_pos,
                );
                if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
                    request.execution_phase = HaulExecutionPhase::TravelingToDestination;
                }
                return;
            }
            let carried = carried_quantity(world, worker_inventory, &request_snapshot.item_id);
            if carried == 0 {
                if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
                    request.execution_phase = HaulExecutionPhase::TravelingToSource;
                }
                return;
            }
            if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
                request.execution_phase = HaulExecutionPhase::Depositing;
            }
            match deposit_haul_cargo(world, request_id, worker_inventory, carried, inventory_ctx) {
                Ok(moved) => {
                    report.deposits += moved;
                }
                Err(reason) => {
                    block_request(
                        world,
                        request_id,
                        &request_snapshot.item_id,
                        reason,
                        simulation_tick,
                    );
                    cancel_unit_task(
                        world,
                        unit_id,
                        TaskCancelReason::Invalidated,
                        &mut Vec::new(),
                    );
                    report.cancellations += 1;
                    return;
                }
            }
        }
        HaulExecutionPhase::Depositing => {
            let carried = carried_quantity(world, worker_inventory, &request_snapshot.item_id);
            if carried > 0 {
                let _ =
                    deposit_haul_cargo(world, request_id, worker_inventory, carried, inventory_ctx);
            }
        }
        HaulExecutionPhase::Completed | HaulExecutionPhase::Failed => {}
    }

    if world
        .hauling_request_store()
        .get(request_id)
        .is_some_and(|request| request.status == HaulingRequestStatus::Completed)
    {
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            task.state = TaskState::Completed;
        }
        report.completions += 1;
    } else if let Some(task) = world.task_store_mut().get_mut(task_id) {
        task.state = TaskState::InProgress;
    }
}

fn haul_batch_quantity(
    world: &WorldData,
    request_id: HaulingRequestId,
    worker_inventory: crate::world::InventoryId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> u32 {
    let Some(request) = world.hauling_request_store().get(request_id) else {
        return 0;
    };
    let reserved_for_request = world
        .inventory_reservation_store()
        .request_record(request_id)
        .and_then(|record| record.source.as_ref().map(|source| source.quantity))
        .unwrap_or(0);
    let available = super::reservation::available_stack_quantity(
        world.inventory_store(),
        world.inventory_reservation_store(),
        request.source_inventory_id,
        &request.item_id,
    )
    .saturating_add(reserved_for_request);
    if available == 0 {
        return 0;
    }
    let worker_capacity = world
        .inventory_store()
        .get(worker_inventory)
        .map(|record| max_accept_stack_quantity(record, inventory_ctx, &request.item_id))
        .unwrap_or(0);
    if worker_capacity == 0 {
        return 0;
    }
    available
        .min(request.remaining_quantity)
        .min(worker_capacity)
        .max(1)
}

fn carried_quantity(
    world: &WorldData,
    inventory_id: crate::world::InventoryId,
    item_id: &crate::world::ItemDefinitionId,
) -> u32 {
    world
        .inventory_store()
        .get(inventory_id)
        .map(|record| crate::world::inventory::count_stack_item(record, item_id))
        .unwrap_or(0)
}

fn block_request(
    world: &mut WorldData,
    request_id: HaulingRequestId,
    item_id: &crate::world::ItemDefinitionId,
    reason: HaulingBlockingReason,
    simulation_tick: u64,
) {
    release_request_reservations(world.inventory_reservation_store_mut(), request_id, item_id);
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.status = HaulingRequestStatus::Blocked;
        request.blocking_reason = Some(reason);
        request.blocked_at_tick = Some(simulation_tick);
        request.assigned_unit_id = None;
        request.assigned_task_id = None;
        request.reservation_state = HaulingReservationState::None;
        world
            .hauling_request_store_mut()
            .refresh_open_key(request_id);
    }
}

/// Release a haul assignment that cannot execute (invalid request id or missing row).
fn recover_orphaned_haul_assignment(world: &mut WorldData, request_id: HaulingRequestId) {
    let Some(item_id) = world
        .hauling_request_store()
        .get(request_id)
        .map(|request| request.item_id.clone())
    else {
        return;
    };
    release_request_reservations(
        world.inventory_reservation_store_mut(),
        request_id,
        &item_id,
    );
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.status = HaulingRequestStatus::Pending;
        request.blocking_reason = None;
        request.blocked_at_tick = None;
        request.assigned_unit_id = None;
        request.assigned_task_id = None;
        request.reservation_state = HaulingReservationState::None;
        request.execution_phase = HaulExecutionPhase::Pending;
        world
            .hauling_request_store_mut()
            .refresh_open_key(request_id);
    }
}

fn ensure_traveling_toward(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    passability: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    target: WorldPosition,
) {
    let already_moving = world.get_unit(unit_id).is_some_and(|unit| {
        matches!(
            unit.state,
            UnitState::Moving {
                target: current, ..
            } if positions_match(current, target)
        )
    });
    if already_moving {
        return;
    }
    let _ = start_unit_move_to(
        world,
        unit_catalog,
        passability,
        nav_config,
        unit_id,
        target,
    );
}

fn positions_match(a: WorldPosition, b: WorldPosition) -> bool {
    a.chunk == b.chunk && a.local == b.local
}

fn within_range(
    unit_pos: WorldPosition,
    target_pos: WorldPosition,
    layout: crate::world::ChunkLayout,
) -> bool {
    let unit_global = unit_pos.to_global(layout);
    let target_global = target_pos.to_global(layout);
    let dx = unit_global.x - target_global.x;
    let dz = unit_global.z - target_global.z;
    (dx * dx + dz * dz).sqrt() <= INTERACTION_WORK_RANGE_METERS
}

fn inventory_building_position(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    inventory_id: crate::world::InventoryId,
) -> Option<WorldPosition> {
    for building_id in world.building_inventory_binding_store().building_ids() {
        let set = world.building_inventory_binding_store().get(building_id)?;
        if set
            .bindings()
            .iter()
            .any(|binding| binding.inventory_id == inventory_id)
        {
            let record = world.get_building(building_id)?;
            if let Some(definition) = building_catalog.get(&record.definition_id) {
                if let Some(profile) = interaction_catalog.profile_for_definition(definition) {
                    if let Some(point_key) = definition.inventory_interaction_point_key.as_deref() {
                        if let Some(point) = profile.points.iter().find(|p| p.key == point_key) {
                            return Some(interaction_point_world_position(
                                record,
                                world.layout(),
                                point,
                            ));
                        }
                    }
                    if let Some(point) = profile.points.first() {
                        return Some(interaction_point_world_position(
                            record,
                            world.layout(),
                            point,
                        ));
                    }
                }
            }
            return Some(record.placement.position);
        }
    }
    None
}
