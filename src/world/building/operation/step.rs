//! Workstation operation stepping (ADR-105 TF5).

use crate::world::UnitId;
use crate::world::building::field_response::EfficiencyBasisPoints;
use crate::world::building::operation::{
    BASE_OPERATION_PROGRESS_PER_TICK, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    scale_progress,
};
use crate::world::building::operational_efficiency::{
    OperationalLimitingFactor, building_operational_efficiency,
};
use crate::world::{BuildingCatalog, BuildingId, WorldData};

use super::error::{OperationCompletionReport, OperationError, OperationStepReport};
use super::params::BuildingOperationParams;
use super::store::BuildingOperationStore;

/// Apply one fixed-tick workstation labor contribution (ADR-105 TF5).
pub fn step_workstation_operation(
    world: &WorldData,
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

    let efficiency = {
        let mut efficiency_ctx = operation.efficiency_context(world, building_catalog);
        building_operational_efficiency(&mut efficiency_ctx, building_id).map_err(|_| {
            OperationError::OperationBlocked(OperationalLimitingFactor::MissingTerrainAssessment)
        })?
    };

    if !efficiency.can_operate {
        return Ok(OperationStepReport {
            building_id,
            worker_id,
            base_progress: BASE_OPERATION_PROGRESS_PER_TICK,
            terrain_efficiency_bp: efficiency.terrain_efficiency_basis_points.value(),
            final_efficiency_bp: efficiency.final_output_efficiency_basis_points.value(),
            scaled_progress: 0,
            accumulated_progress: operation
                .operation_store
                .get(building_id)
                .map(|state| state.progress.value())
                .unwrap_or(0),
            completions: 0,
            can_operate: false,
            limiting_factor: efficiency.limiting_factor.clone(),
        });
    }

    let final_bp = efficiency.final_output_efficiency_basis_points.value();
    let scaled = scale_progress(BASE_OPERATION_PROGRESS_PER_TICK, final_bp)
        .map_err(|_| OperationError::OperationProgressOverflow)?;

    let state = operation.operation_store.get_or_default_mut(building_id);
    state.last_efficiency_revision = efficiency.assessment_revision;
    state.progress = state
        .progress
        .add_scaled_base(BASE_OPERATION_PROGRESS_PER_TICK, final_bp)
        .map_err(|_| OperationError::OperationProgressOverflow)?;
    let completions = state
        .progress
        .completions_since(PRODUCTION_PROGRESS_ONE_UNIT);
    state.completion_count = state.completion_count.saturating_add(completions);

    Ok(OperationStepReport {
        building_id,
        worker_id,
        base_progress: BASE_OPERATION_PROGRESS_PER_TICK,
        terrain_efficiency_bp: efficiency.terrain_efficiency_basis_points.value(),
        final_efficiency_bp: final_bp,
        scaled_progress: scaled,
        accumulated_progress: state.progress.value(),
        completions,
        can_operate: true,
        limiting_factor: OperationalLimitingFactor::None,
    })
}

/// Dev/test helper: apply N fixed ticks without worker validation.
pub fn apply_operation_ticks(
    world: &WorldData,
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
    let leftover = operation
        .operation_store
        .get(building_id)
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
