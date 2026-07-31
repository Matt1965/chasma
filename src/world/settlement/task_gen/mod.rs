//! Strategic Task Generation (SA6 / ADR-121).
//!
//! SettlementIntent → authored templates → TaskStore.
//! Never assigns workers. Never emits production/haul tasks.

mod catalog;
mod emit;
mod report;
mod step;
mod store;
mod template;
mod validation;

#[cfg(test)]
mod tests;

pub use catalog::{StrategicTaskCatalogError, StrategicTaskTemplateCatalog};
pub use emit::{
    StrategicTaskGenContext, generate_strategic_tasks_for_settlement, intent_to_task_priority,
};
pub use report::{StrategicTaskEmission, StrategicTaskGenerationReport};
pub use step::{
    STRATEGIC_TASK_GEN_CADENCE_TICKS, generate_strategic_tasks_now,
    step_settlement_strategic_task_generation,
};
pub use store::StrategicTaskGenerationStore;
pub use template::{
    StrategicTaskTemplate, StrategicTaskTemplateId, starter_strategic_task_templates,
};
pub use validation::{StrategicTaskValidationError, validate_strategic_task_report};
