//! Workstation operation stepping (ADR-105 TF5, EP2 production runtime).

use crate::world::UnitId;
use crate::world::building::operation::{
    BASE_OPERATION_PROGRESS_PER_TICK, PRODUCTION_PROGRESS_ONE_UNIT, scale_progress,
    workstation_workers_for_building,
};
use crate::world::building::inventory_binding::validate_selected_operation_inventory_bindings;
use crate::world::building::operational_efficiency::{
    OperationalLimitingFactor, building_operational_efficiency,
};
use crate::world::{BuildingCatalog, BuildingId, WorldData};

use super::error::{OperationCompletionReport, OperationError, OperationStepReport};
use super::execute::execute_production_cycle;
use super::execute::assess_production_execution;
use crate::world::operation::OperationOutputDefinition;
use crate::world::{
    sync_logistics_requests_from_assessment, sync_output_surplus_after_production,
};
use super::lifecycle::{OperationLifecycle, set_blocked};
use super::params::BuildingOperationParams;

/// Apply one fixed-tick workstation labor contribution (ADR-105 TF5, EP1).
pub fn step_workstation_operation(
    world: &mut WorldData,
    operation: &mut BuildingOperationParams<'_>,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    worker_id: UnitId,
) -> Result<OperationStepReport, OperationError> {
    if world.get_building(building_id).is_none() {
        return Err(OperationError::BuildingNotFound(building_id));
    }
    if world.get_unit(worker_id).is_none() {
        return Err(OperationError::WorkerNotFound(worker_id));
    }

    let definition = world
        .get_building(building_id)
        .and_then(|record| building_catalog.get(&record.definition_id).cloned());

    if let Some(definition) = definition.as_ref() {
        world
            .building_production_store_mut()
            .ensure_policy_for_building(
                building_id,
                definition,
                operation.operation_catalog,
            );
    }

    let policy_snapshot = world
        .building_production_store()
        .get_policy(building_id)
        .cloned()
        .unwrap_or_default();
    let active_workers = workstation_workers_for_building(world, building_id).len() as u32;

    if !policy_snapshot.enabled {
        {
            let state = world.building_production_store_mut().get_state_mut(building_id);
            state.lifecycle = OperationLifecycle::Disabled;
            state.blocked_reason = None;
            state.active_worker_count = active_workers;
        }
        return Ok(blocked_step_report(
            world,
            building_id,
            worker_id,
            OperationalLimitingFactor::BuildingDisabled,
            policy_snapshot.selected_operation.clone(),
        ));
    }

    if policy_snapshot.paused {
        {
            let state = world.building_production_store_mut().get_state_mut(building_id);
            state.lifecycle = OperationLifecycle::Paused;
            state.blocked_reason = Some(OperationalLimitingFactor::Paused);
            state.active_worker_count = active_workers;
        }
        return Ok(blocked_step_report(
            world,
            building_id,
            worker_id,
            OperationalLimitingFactor::Paused,
            policy_snapshot.selected_operation.clone(),
        ));
    }

    if policy_snapshot.selected_operation.is_none() {
        {
            let state = world.building_production_store_mut().get_state_mut(building_id);
            state.lifecycle = OperationLifecycle::Idle;
            state.blocked_reason = Some(OperationalLimitingFactor::InvalidOperation);
            state.active_worker_count = active_workers;
        }
        return Ok(blocked_step_report(
            world,
            building_id,
            worker_id,
            OperationalLimitingFactor::InvalidOperation,
            None,
        ));
    }

    if let Some(selected) = policy_snapshot.selected_operation.as_ref() {
        let invalid = operation.operation_catalog.get(selected).is_none()
            || definition
                .as_ref()
                .is_some_and(|def| !def.supports_operation(selected));
        if invalid {
            {
                let state = world.building_production_store_mut().get_state_mut(building_id);
                state.lifecycle = OperationLifecycle::Blocked;
                state.blocked_reason = Some(OperationalLimitingFactor::InvalidOperation);
                state.active_worker_count = active_workers;
            }
            return Ok(blocked_step_report(
                world,
                building_id,
                worker_id,
                OperationalLimitingFactor::InvalidOperation,
                policy_snapshot.selected_operation.clone(),
            ));
        }

        if let Some(op_def) = operation.operation_catalog.get(selected) {
            if let Some(def) = definition.as_ref() {
                if validate_selected_operation_inventory_bindings(
                    op_def,
                    def,
                    building_id,
                    world.building_inventory_binding_store(),
                )
                .is_err()
                {
                    {
                        let state =
                            world.building_production_store_mut().get_state_mut(building_id);
                        state.lifecycle = OperationLifecycle::Blocked;
                        state.blocked_reason =
                            Some(OperationalLimitingFactor::InvalidInventoryBinding);
                        state.active_worker_count = active_workers;
                    }
                    return Ok(blocked_step_report(
                        world,
                        building_id,
                        worker_id,
                        OperationalLimitingFactor::InvalidInventoryBinding,
                        policy_snapshot.selected_operation.clone(),
                    ));
                }
            }
        }
    }

    let completion_count = world
        .building_production_store()
        .get_state(building_id)
        .map(|state| state.completion_count)
        .unwrap_or(0);
    if policy_snapshot
        .repeat_mode
        .is_exhausted(completion_count)
    {
        {
            let state = world.building_production_store_mut().get_state_mut(building_id);
            state.lifecycle = OperationLifecycle::Completed;
            state.blocked_reason = Some(OperationalLimitingFactor::InvalidOperation);
            state.active_worker_count = active_workers;
        }
        return Ok(blocked_step_report(
            world,
            building_id,
            worker_id,
            OperationalLimitingFactor::InvalidOperation,
            policy_snapshot.selected_operation.clone(),
        ));
    }

    let selected_operation = policy_snapshot
        .selected_operation
        .as_ref()
        .and_then(|id| operation.operation_catalog.get(id));

    let efficiency = {
        let mut efficiency_ctx = operation.efficiency_context(world, building_catalog);
        building_operational_efficiency(
            &mut efficiency_ctx,
            building_id,
            selected_operation,
        )
        .map_err(|_| {
            OperationError::OperationBlocked(OperationalLimitingFactor::MissingTerrainAssessment)
        })?
    };

    if !efficiency.can_operate {
        {
            let state = world.building_production_store_mut().get_state_mut(building_id);
            set_blocked(
                &mut state.lifecycle,
                &mut state.blocked_reason,
                efficiency.limiting_factor.clone(),
            );
            state.active_worker_count = active_workers;
        }
        return Ok(blocked_step_report(
            world,
            building_id,
            worker_id,
            efficiency.limiting_factor,
            policy_snapshot.selected_operation.clone(),
        ));
    }

    let final_bp = efficiency.final_output_efficiency_basis_points.value();
    let scaled = scale_progress(BASE_OPERATION_PROGRESS_PER_TICK, final_bp)
        .map_err(|_| OperationError::OperationProgressOverflow)?;

    let selected_operation_id = policy_snapshot.selected_operation.clone().expect("validated");
    let operation_definition = operation
        .operation_catalog
        .get(&selected_operation_id)
        .cloned()
        .expect("validated");
    let building_definition = definition.clone().expect("validated");

    let (accumulated_progress, completions, lifecycle, blocked_reason) = {
        let state = world.building_production_store_mut().get_state_mut(building_id);
        state.last_efficiency_revision = efficiency.assessment_revision;
        state.lifecycle = OperationLifecycle::Running;
        state.blocked_reason = None;
        state.active_worker_count = active_workers;
        state.progress = state
            .progress
            .add_scaled_base(BASE_OPERATION_PROGRESS_PER_TICK, final_bp)
            .map_err(|_| OperationError::OperationProgressOverflow)?;

        let mut executed_completions = 0u32;
        let mut final_lifecycle = OperationLifecycle::Running;

        loop {
            let ready = world
                .building_production_store()
                .get_state(building_id)
                .is_some_and(|state| state.progress.value() >= PRODUCTION_PROGRESS_ONE_UNIT);
            if !ready {
                break;
            }

            match execute_production_cycle(
                world,
                operation.inventory_ctx,
                building_id,
                &operation_definition,
                &building_definition,
            ) {
                Ok(()) => {
                    let completion_count = {
                        let state =
                            world.building_production_store_mut().get_state_mut(building_id);
                        state
                            .progress
                            .completions_since(PRODUCTION_PROGRESS_ONE_UNIT);
                        state.completion_count = state.completion_count.saturating_add(1);
                        state.completion_count
                    };
                    executed_completions = executed_completions.saturating_add(1);
                    for output in &operation_definition.outputs {
                        if let OperationOutputDefinition::Item { item_id, .. } = output {
                            sync_output_surplus_after_production(
                                world,
                                building_catalog,
                                building_id,
                                item_id,
                                0,
                                operation.inventory_ctx,
                            );
                        }
                    }
                    if policy_snapshot
                        .repeat_mode
                        .is_exhausted(completion_count)
                    {
                        final_lifecycle = OperationLifecycle::Completed;
                        break;
                    }
                }
                Err(limiting_factor) => {
                    let state = world.building_production_store_mut().get_state_mut(building_id);
                    set_blocked(
                        &mut state.lifecycle,
                        &mut state.blocked_reason,
                        limiting_factor.clone(),
                    );
                    final_lifecycle = OperationLifecycle::Blocked;
                    let assessment = assess_production_execution(
                        world,
                        operation.inventory_ctx,
                        building_id,
                        &operation_definition,
                        &building_definition,
                    );
                    sync_logistics_requests_from_assessment(
                        world,
                        building_catalog,
                        building_id,
                        &assessment,
                        0,
                        operation.inventory_ctx,
                    );
                    let _ = limiting_factor;
                    break;
                }
            }
        }

        let state = world.building_production_store_mut().get_state_mut(building_id);
        if final_lifecycle == OperationLifecycle::Running
            && policy_snapshot
                .repeat_mode
                .is_exhausted(state.completion_count)
        {
            final_lifecycle = OperationLifecycle::Completed;
        }
        state.lifecycle = final_lifecycle;
        let blocked_reason = state.blocked_reason.clone();
        (
            state.progress.value(),
            executed_completions,
            state.lifecycle,
            blocked_reason,
        )
    };

    Ok(OperationStepReport {
        building_id,
        worker_id,
        base_progress: BASE_OPERATION_PROGRESS_PER_TICK,
        terrain_efficiency_bp: efficiency.terrain_efficiency_basis_points.value(),
        final_efficiency_bp: final_bp,
        scaled_progress: scaled,
        accumulated_progress,
        completions,
        can_operate: lifecycle.accepts_labor(),
        limiting_factor: blocked_reason.unwrap_or(OperationalLimitingFactor::None),
        lifecycle,
        selected_operation: policy_snapshot.selected_operation,
    })
}

fn blocked_step_report(
    world: &WorldData,
    building_id: BuildingId,
    worker_id: UnitId,
    limiting_factor: OperationalLimitingFactor,
    selected_operation: Option<super::operation_id::OperationDefinitionId>,
) -> OperationStepReport {
    let (accumulated_progress, lifecycle) = world
        .building_production_store()
        .get_state(building_id)
        .map(|state| (state.progress.value(), state.lifecycle))
        .unwrap_or((0, OperationLifecycle::Idle));
    OperationStepReport {
        building_id,
        worker_id,
        base_progress: BASE_OPERATION_PROGRESS_PER_TICK,
        terrain_efficiency_bp: 0,
        final_efficiency_bp: 0,
        scaled_progress: 0,
        accumulated_progress,
        completions: 0,
        can_operate: false,
        limiting_factor,
        lifecycle,
        selected_operation,
    }
}

/// Dev/test helper: apply N fixed ticks without worker validation.
pub fn apply_operation_ticks(
    world: &mut WorldData,
    operation: &mut BuildingOperationParams<'_>,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    worker_id: UnitId,
    ticks: u32,
) -> Result<OperationCompletionReport, OperationError> {
    let mut total_completions = 0u32;
    for _ in 0..ticks {
        let report =
            step_workstation_operation(world, operation, building_catalog, building_id, worker_id)?;
        total_completions = total_completions.saturating_add(report.completions);
        if !report.can_operate {
            return Ok(OperationCompletionReport {
                building_id,
                completed_units: total_completions,
                leftover_progress: report.accumulated_progress,
                blocked: true,
                blocked_reason: Some(report.limiting_factor),
            });
        }
    }
    let leftover = world
        .building_production_store()
        .get_state(building_id)
        .map(|state| state.progress.value())
        .unwrap_or(0);
    Ok(OperationCompletionReport {
        building_id,
        completed_units: total_completions,
        leftover_progress: leftover,
        blocked: false,
        blocked_reason: None,
    })
}

/// Expected ticks to complete one unit at a given efficiency (deterministic ceiling).
pub fn expected_ticks_to_complete(efficiency_basis_points: u32) -> u64 {
    if efficiency_basis_points == 0 {
        return u64::MAX;
    }
    let scaled =
        scale_progress(BASE_OPERATION_PROGRESS_PER_TICK, efficiency_basis_points).unwrap_or(0);
    if scaled == 0 {
        return u64::MAX;
    }
    (PRODUCTION_PROGRESS_ONE_UNIT + scaled - 1) / scaled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;

    #[test]
    fn expected_ticks_half_double_one_and_half() {
        let full = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT);
        let half = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT / 2);
        let rich = expected_ticks_to_complete(15_000);
        assert_eq!(half, full * 2);
        assert_eq!(rich * 3 / 2, full);
    }
}
