mod commands;
mod error;
mod execute;
mod inventory_role;
mod lifecycle;
mod operation_id;
mod params;
mod policy;
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
mod save_tests;
#[cfg(test)]
mod tests;

pub use commands::{
    ProductionCommandError, cycle_production_selected_operation, production_policy,
    reset_production_progress, set_production_enabled, set_production_execution_mode,
    set_production_paused, set_production_repeat_count, set_production_selected_operation,
};
pub use error::{OperationCompletionReport, OperationError, OperationStepReport};
pub use execute::{
    ProductionExecutionAssessment, ProductionExecutionFailure, ResolvedProductionInput,
    ResolvedProductionOutput, assess_production_execution, execute_production_cycle,
};
pub use inventory_role::{BuildingInventoryBinding, BuildingInventoryRole};
pub use lifecycle::OperationLifecycle;
pub use operation_id::{OperationDefinitionId, OperationId};
pub use params::BuildingOperationParams;
pub use policy::{BuildingOperationPolicy, ControlSource, RepeatMode};
pub use progress::{
    BASE_OPERATION_PROGRESS_PER_TICK, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    scale_progress,
};
pub use query::workstation_workers_for_building;
pub use step::{apply_operation_ticks, expected_ticks_to_complete, step_workstation_operation};
pub use store::{
    BuildingOperationSaveState, BuildingOperationState, BuildingOperationStore,
    BuildingProductionSaveState, BuildingProductionStore,
};
pub use validation::{
    PRODUCTION_STEPPING_MODEL, ProductionValidationIssue, validate_production_runtime,
    validate_production_runtime_with_catalogs,
};
