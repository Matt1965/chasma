mod error;
mod params;
mod progress;
mod step;
mod store;

#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod tests;

pub use error::{OperationCompletionReport, OperationError, OperationStepReport};
pub use params::BuildingOperationParams;
pub use progress::{
    BASE_OPERATION_PROGRESS_PER_TICK, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    scale_progress,
};
pub use step::{apply_operation_ticks, expected_ticks_to_complete, step_workstation_operation};
pub use store::{BuildingOperationSaveState, BuildingOperationState, BuildingOperationStore};
