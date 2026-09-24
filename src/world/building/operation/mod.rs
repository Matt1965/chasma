mod commands;
mod error;
mod execute;
mod farm;
mod inventory_role;
mod lifecycle;
mod operation_id;
mod params;
mod player_policy;
mod policy;
mod priority;
mod progress;
mod query;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod ep2_tests;
#[cfg(test)]
mod ep8_chain_tests;
#[cfg(test)]
mod execute_tests;
#[cfg(test)]
mod extraction_tests;
#[cfg(test)]
mod farm_tests;
#[cfg(test)]
mod farm_worker_persistence_tests;
#[cfg(test)]
mod phase4_content_tests;
#[cfg(test)]
mod priority_tests;
#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod tests;

pub use commands::{
    ProductionCommandError, cycle_production_selected_operation,
    reset_production_progress, set_building_work_priority, set_production_enabled,
    set_production_execution_mode, set_production_paused, set_production_repeat_count,
    set_production_selected_operation,
};
pub use execute::{
    ProductionExecutionAssessment, ProductionExecutionFailure, assess_production_execution, execute_production_cycle,
};
pub use farm::{
    FarmProductionPhase, farm_growth_percent, farm_harvest_percent,
    farm_needs_harvest_worker, is_prispod_farm_definition,
    progress_to_percent, reconcile_farm_harvest_phase, step_all_farm_passive_growth,
};
pub use lifecycle::OperationLifecycle;
pub use operation_id::OperationDefinitionId;
pub use params::BuildingOperationParams;
pub use player_policy::{
    apply_player_building_work_priority, apply_player_production_enabled,
    apply_player_production_selected_operation,
};
pub use policy::{BuildingOperationPolicy, ControlSource, RepeatMode};
pub use priority::{
    BuildingWorkPriorityLevel,
    building_work_priority_label, building_work_priority_level, building_work_priority_to_task_priority,
    building_work_priority_to_task_priority_for_building, building_work_priority_u8,
    building_work_priority_u8_for_level,
};
pub use progress::{
    BASE_OPERATION_PROGRESS_PER_TICK, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    scale_progress,
};
pub use query::workstation_workers_for_building;
#[cfg(test)]
pub use farm::grow_prispods_operation_id;
#[cfg(test)]
pub use step::{apply_operation_ticks, expected_ticks_to_complete};
pub use step::step_workstation_operation;
pub use store::{
    BuildingOperationSaveState, BuildingOperationState, BuildingOperationStore,
    BuildingProductionSaveState, BuildingProductionStore,
};
pub use validation::{
    PRODUCTION_STEPPING_MODEL, ProductionValidationIssue, validate_production_runtime,
    validate_production_runtime_with_catalogs,
};
