//! Prispod Farm closed-loop production: passive growth + worker harvest.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::inventory_binding::validate_selected_operation_inventory_bindings;
use crate::world::building::operational_efficiency::{
    OperationalLimitingFactor, building_operational_efficiency,
};
use crate::world::operation::OperationOutputDefinition;
use crate::world::{
    BuildingCatalog, BuildingId, UnitId, WorldData, sync_logistics_requests_from_assessment,
    sync_output_surplus_after_production,
};

use super::error::{OperationError, OperationStepReport};
use super::execute::{assess_production_execution, execute_production_cycle};
use super::lifecycle::{OperationLifecycle, set_blocked};
use super::operation_id::OperationDefinitionId;
use super::params::BuildingOperationParams;
use super::policy::BuildingOperationPolicy;
use super::progress::{
    BASE_OPERATION_PROGRESS_PER_TICK, PRODUCTION_PROGRESS_ONE_UNIT, scale_progress,
};
use super::query::workstation_workers_for_building;
use super::store::BuildingProductionStore;

pub const PRISPOD_FARM_DEFINITION_ID: &str = "prispod_farm";
pub const GROW_PRISPODS_OPERATION_ID: &str = "grow_prispods";

/// Farm crop lifecycle phases (Prispod Farm only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect, Serialize, Deserialize)]
pub enum FarmProductionPhase {
    #[default]
    Growing,
    ReadyToHarvest,
    Harvesting,
}

/// Per-farm runtime state separate from generic workstation progress.
#[derive(Debug, Clone, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub struct FarmProductionState {
    pub phase: FarmProductionPhase,
    pub growth_progress: super::progress::ProductionProgress,
    pub harvest_progress: super::progress::ProductionProgress,
}

pub fn is_prispod_farm_definition(definition: &BuildingDefinition) -> bool {
    definition.id.as_str() == PRISPOD_FARM_DEFINITION_ID
}

pub fn grow_prispods_operation_id() -> OperationDefinitionId {
    OperationDefinitionId::new(GROW_PRISPODS_OPERATION_ID)
}

pub fn farm_needs_harvest_worker(
    store: &BuildingProductionStore,
    building_id: BuildingId,
    definition: &BuildingDefinition,
) -> bool {
    if !is_prispod_farm_definition(definition) {
        return false;
    }
    store.farm_state(building_id).is_some_and(|state| {
        matches!(
            state.phase,
            FarmProductionPhase::ReadyToHarvest | FarmProductionPhase::Harvesting
        )
    })
}

pub fn farm_growth_percent(store: &BuildingProductionStore, building_id: BuildingId) -> u32 {
    store
        .farm_state(building_id)
        .map(|state| progress_to_percent(state.growth_progress.value()))
        .unwrap_or(0)
}

pub fn farm_harvest_percent(store: &BuildingProductionStore, building_id: BuildingId) -> u32 {
    store
        .farm_state(building_id)
        .map(|state| progress_to_percent(state.harvest_progress.value()))
        .unwrap_or(0)
}

pub fn progress_to_percent(progress: u64) -> u32 {
    ((progress as u128 * 100) / PRODUCTION_PROGRESS_ONE_UNIT as u128) as u32
}

/// Passive crop growth for all operational Prispod Farms (no worker required).
pub fn step_all_farm_passive_growth(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation: &mut BuildingOperationParams<'_>,
) {
    let building_ids = world.sorted_building_ids();
    for building_id in building_ids {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if !crate::world::building_accepts_workstation_use(record) {
            continue;
        }
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if !is_prispod_farm_definition(definition) {
            continue;
        }

        {
            let store = world.building_production_store_mut();
            store.ensure_policy_for_building(building_id, definition, operation.operation_catalog);
        }
        let policy = world
            .building_production_store()
            .get_policy(building_id)
            .cloned()
            .unwrap_or_default();
        if !farm_policy_allows_growth(&policy, definition) {
            sync_farm_idle_state(world, building_id, &policy, 0);
            continue;
        }

        let phase = world
            .building_production_store()
            .farm_state(building_id)
            .map(|farm| farm.phase)
            .unwrap_or(FarmProductionPhase::Growing);
        if phase != FarmProductionPhase::Growing {
            let farm = world
                .building_production_store()
                .farm_state(building_id)
                .cloned()
                .unwrap_or_default();
            sync_farm_runtime_lifecycle(world, building_id, &farm, &policy, 0);
            update_farm_output_block_state(
                world,
                building_id,
                definition,
                operation,
                &farm,
                &policy,
            );
            continue;
        }

        let selected_operation = farm_effective_operation(&policy, definition);
        let operation_definition = selected_operation
            .as_ref()
            .and_then(|id| operation.operation_catalog.get(id));
        let efficiency = if let Some(op_def) = operation_definition {
            let mut efficiency_ctx = operation.efficiency_context(world, building_catalog);
            building_operational_efficiency(&mut efficiency_ctx, building_id, Some(op_def)).ok()
        } else {
            None
        };
        let Some(efficiency) = efficiency else {
            sync_farm_idle_state(world, building_id, &policy, 0);
            continue;
        };
        if !efficiency.can_operate {
            let state = world
                .building_production_store_mut()
                .get_state_mut(building_id);
            set_blocked(
                &mut state.lifecycle,
                &mut state.blocked_reason,
                efficiency.limiting_factor.clone(),
            );
            state.active_worker_count = 0;
            continue;
        }
        let final_bp = efficiency.final_output_efficiency_basis_points.value();
        let scaled = scale_progress(BASE_OPERATION_PROGRESS_PER_TICK, final_bp).unwrap_or(0);
        if scaled == 0 {
            continue;
        }
        {
            let store = world.building_production_store_mut();
            let farm = store.farm_state_mut(building_id);
            farm.growth_progress = farm
                .growth_progress
                .add_scaled_base(BASE_OPERATION_PROGRESS_PER_TICK, final_bp)
                .unwrap_or(farm.growth_progress);
            if farm.growth_progress.value() >= PRODUCTION_PROGRESS_ONE_UNIT {
                farm.phase = FarmProductionPhase::ReadyToHarvest;
                farm.harvest_progress = super::progress::ProductionProgress::ZERO;
            }
        }
        let farm = world
            .building_production_store()
            .farm_state(building_id)
            .cloned()
            .unwrap_or_default();
        sync_farm_runtime_lifecycle(world, building_id, &farm, &policy, 0);
        update_farm_output_block_state(world, building_id, definition, operation, &farm, &policy);
    }
}

/// Worker-driven harvest labor for Prispod Farm (growth is passive).
pub fn step_farm_harvest_operation(
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
        .and_then(|record| building_catalog.get(&record.definition_id).cloned())
        .expect("farm harvest requires definition");
    if !is_prispod_farm_definition(&definition) {
        return Err(OperationError::OperationBlocked(
            OperationalLimitingFactor::InvalidOperation,
        ));
    }

    world
        .building_production_store_mut()
        .ensure_policy_for_building(building_id, &definition, operation.operation_catalog);

    let policy_snapshot = world
        .building_production_store()
        .get_policy(building_id)
        .cloned()
        .unwrap_or_default();
    let active_workers = workstation_workers_for_building(world, building_id).len() as u32;

    if let Some(blocked) = farm_policy_block(&policy_snapshot, &definition) {
        sync_farm_idle_state(world, building_id, &policy_snapshot, active_workers);
        return Ok(farm_blocked_report(
            world,
            building_id,
            worker_id,
            blocked,
            policy_snapshot.selected_operation.clone(),
        ));
    }

    let selected_operation_id = farm_effective_operation(&policy_snapshot, &definition)
        .filter(|id| operation.operation_catalog.get(id).is_some())
        .filter(|id| definition.supports_operation(id))
        .ok_or(OperationError::OperationBlocked(
            OperationalLimitingFactor::InvalidOperation,
        ))?;
    let operation_definition = operation
        .operation_catalog
        .get(&selected_operation_id)
        .cloned()
        .expect("validated");
    validate_selected_operation_inventory_bindings(
        &operation_definition,
        &definition,
        building_id,
        world.building_inventory_binding_store(),
    )
    .map_err(|_| {
        OperationError::OperationBlocked(OperationalLimitingFactor::InvalidInventoryBinding)
    })?;

    let efficiency = {
        let mut efficiency_ctx = operation.efficiency_context(world, building_catalog);
        building_operational_efficiency(
            &mut efficiency_ctx,
            building_id,
            Some(&operation_definition),
        )
        .map_err(|_| {
            OperationError::OperationBlocked(OperationalLimitingFactor::MissingTerrainAssessment)
        })?
    };
    if !efficiency.can_operate {
        {
            let state = world
                .building_production_store_mut()
                .get_state_mut(building_id);
            set_blocked(
                &mut state.lifecycle,
                &mut state.blocked_reason,
                efficiency.limiting_factor.clone(),
            );
            state.active_worker_count = active_workers;
        }
        return Ok(farm_blocked_report(
            world,
            building_id,
            worker_id,
            efficiency.limiting_factor,
            Some(selected_operation_id),
        ));
    }

    let farm_phase = world
        .building_production_store()
        .farm_state(building_id)
        .map(|state| state.phase)
        .unwrap_or(FarmProductionPhase::Growing);
    if farm_phase == FarmProductionPhase::Growing {
        return Ok(farm_blocked_report(
            world,
            building_id,
            worker_id,
            OperationalLimitingFactor::None,
            Some(selected_operation_id),
        ));
    }

    let final_bp = efficiency.final_output_efficiency_basis_points.value();
    let scaled = scale_progress(BASE_OPERATION_PROGRESS_PER_TICK, final_bp)
        .map_err(|_| OperationError::OperationProgressOverflow)?;

    let harvest_ready = {
        let store = world.building_production_store_mut();
        let farm = store.farm_state_mut(building_id);
        if farm.phase == FarmProductionPhase::ReadyToHarvest {
            farm.phase = FarmProductionPhase::Harvesting;
        }
        farm.harvest_progress = farm
            .harvest_progress
            .add_scaled_base(BASE_OPERATION_PROGRESS_PER_TICK, final_bp)
            .map_err(|_| OperationError::OperationProgressOverflow)?;
        farm.harvest_progress.value() >= PRODUCTION_PROGRESS_ONE_UNIT
    };

    let cycle_result = if harvest_ready {
        execute_production_cycle(
            world,
            operation.inventory_ctx,
            building_id,
            &operation_definition,
            &definition,
        )
    } else {
        Ok(())
    };

    let mut executed_completions = 0u32;
    let mut final_lifecycle = OperationLifecycle::Running;
    let mut blocked_reason_out = None;

    if harvest_ready {
        match &cycle_result {
            Ok(()) => {
                executed_completions = 1;
                let store = world.building_production_store_mut();
                {
                    let farm = store.farm_state_mut(building_id);
                    farm.phase = FarmProductionPhase::Growing;
                    farm.growth_progress = super::progress::ProductionProgress::ZERO;
                    farm.harvest_progress = super::progress::ProductionProgress::ZERO;
                }
                let completion_count = store
                    .get_state(building_id)
                    .map(|state| state.completion_count.saturating_add(1))
                    .unwrap_or(1);
                store.get_state_mut(building_id).completion_count = completion_count;
            }
            Err(limiting_factor) => {
                let store = world.building_production_store_mut();
                {
                    let farm = store.farm_state_mut(building_id);
                    farm.phase = FarmProductionPhase::ReadyToHarvest;
                    farm.harvest_progress = super::progress::ProductionProgress::ZERO;
                }
                let state = store.get_state_mut(building_id);
                set_blocked(
                    &mut state.lifecycle,
                    &mut state.blocked_reason,
                    limiting_factor.clone(),
                );
                final_lifecycle = OperationLifecycle::Blocked;
                blocked_reason_out = Some(limiting_factor.clone());
            }
        }
    }

    let accumulated_progress = {
        let store = world.building_production_store_mut();
        let farm = store.farm_state(building_id).cloned().unwrap_or_default();
        let state = store.get_state_mut(building_id);
        state.last_efficiency_revision = efficiency.assessment_revision;
        state.lifecycle = final_lifecycle;
        state.active_worker_count = active_workers;
        if blocked_reason_out.is_none() {
            state.blocked_reason = None;
        }
        state.progress = farm.harvest_progress;
        farm.harvest_progress.value()
    };

    let lifecycle = world
        .building_production_store()
        .get_state(building_id)
        .map(|state| state.lifecycle)
        .unwrap_or(final_lifecycle);
    let blocked_reason = world
        .building_production_store()
        .get_state(building_id)
        .and_then(|state| state.blocked_reason.clone());

    if harvest_ready {
        if cycle_result.is_ok() {
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
        } else if let Err(limiting_factor) = &cycle_result {
            let assessment = assess_production_execution(
                world,
                operation.inventory_ctx,
                building_id,
                &operation_definition,
                &definition,
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
        }
    }

    let completions = executed_completions;

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
        selected_operation: Some(selected_operation_id),
    })
}

pub fn reconcile_farm_harvest_phase(
    world: &mut WorldData,
    building_id: BuildingId,
    definition: &BuildingDefinition,
) {
    if !is_prispod_farm_definition(definition) {
        return;
    }
    let active_workers = workstation_workers_for_building(world, building_id).len();
    if active_workers == 0 {
        let store = world.building_production_store_mut();
        let farm = store.farm_state_mut(building_id);
        if farm.phase == FarmProductionPhase::Harvesting {
            farm.phase = FarmProductionPhase::ReadyToHarvest;
        }
    }
}

fn farm_policy_allows_growth(
    policy: &BuildingOperationPolicy,
    definition: &BuildingDefinition,
) -> bool {
    policy.enabled && !policy.paused && farm_effective_operation(policy, definition).is_some()
}

fn farm_effective_operation(
    policy: &BuildingOperationPolicy,
    definition: &BuildingDefinition,
) -> Option<OperationDefinitionId> {
    policy
        .selected_operation
        .clone()
        .or_else(|| definition.resolved_default_operation())
}

fn farm_policy_block(
    policy: &BuildingOperationPolicy,
    definition: &BuildingDefinition,
) -> Option<OperationalLimitingFactor> {
    if !policy.enabled {
        return Some(OperationalLimitingFactor::BuildingDisabled);
    }
    if policy.paused {
        return Some(OperationalLimitingFactor::Paused);
    }
    if farm_effective_operation(policy, definition).is_none() {
        return Some(OperationalLimitingFactor::InvalidOperation);
    }
    None
}

fn sync_farm_idle_state(
    world: &mut WorldData,
    building_id: BuildingId,
    policy: &BuildingOperationPolicy,
    active_workers: u32,
) {
    let farm = world
        .building_production_store()
        .farm_state(building_id)
        .cloned()
        .unwrap_or_default();
    sync_farm_runtime_lifecycle(world, building_id, &farm, policy, active_workers);
}

fn sync_farm_runtime_lifecycle(
    world: &mut WorldData,
    building_id: BuildingId,
    farm: &FarmProductionState,
    policy: &BuildingOperationPolicy,
    active_workers: u32,
) {
    let state = world
        .building_production_store_mut()
        .get_state_mut(building_id);
    state.active_worker_count = active_workers;
    state.progress = match farm.phase {
        FarmProductionPhase::Growing => farm.growth_progress,
        FarmProductionPhase::ReadyToHarvest => farm.growth_progress,
        FarmProductionPhase::Harvesting => farm.harvest_progress,
    };
    if !policy.enabled {
        state.lifecycle = OperationLifecycle::Disabled;
        state.blocked_reason = None;
        return;
    }
    if policy.paused {
        state.lifecycle = OperationLifecycle::Paused;
        state.blocked_reason = Some(OperationalLimitingFactor::Paused);
        return;
    }
    state.lifecycle = match farm.phase {
        FarmProductionPhase::Growing => OperationLifecycle::Running,
        FarmProductionPhase::ReadyToHarvest => OperationLifecycle::Running,
        FarmProductionPhase::Harvesting => OperationLifecycle::Running,
    };
    if state.lifecycle == OperationLifecycle::Running {
        state.blocked_reason = None;
    }
}

fn update_farm_output_block_state(
    world: &mut WorldData,
    building_id: BuildingId,
    definition: &BuildingDefinition,
    operation: &mut BuildingOperationParams<'_>,
    farm: &FarmProductionState,
    policy: &BuildingOperationPolicy,
) {
    if farm.phase != FarmProductionPhase::ReadyToHarvest {
        return;
    }
    let selected_operation_id = farm_effective_operation(policy, definition);
    if selected_operation_id.is_none() {
        return;
    }
    let selected_operation_id = selected_operation_id.unwrap();
    let operation_definition = operation
        .operation_catalog
        .get(&selected_operation_id)
        .cloned();
    let Some(operation_definition) = operation_definition else {
        return;
    };
    let assessment = assess_production_execution(
        world,
        operation.inventory_ctx,
        building_id,
        &operation_definition,
        definition,
    );
    if let Some(failure) = assessment.blocking {
        let state = world
            .building_production_store_mut()
            .get_state_mut(building_id);
        set_blocked(
            &mut state.lifecycle,
            &mut state.blocked_reason,
            failure.limiting_factor(),
        );
    }
}

fn farm_blocked_report(
    world: &WorldData,
    building_id: BuildingId,
    worker_id: UnitId,
    limiting_factor: OperationalLimitingFactor,
    selected_operation: Option<OperationDefinitionId>,
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
