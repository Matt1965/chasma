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
    generate_strategic_tasks_for_settlement, intent_to_task_priority, StrategicTaskGenContext,
};
pub use report::{StrategicTaskEmission, StrategicTaskGenerationReport};
pub use step::{
    generate_strategic_tasks_now, step_settlement_strategic_task_generation,
    STRATEGIC_TASK_GEN_CADENCE_TICKS,
};
pub use store::StrategicTaskGenerationStore;
pub use template::{
    starter_strategic_task_templates, StrategicTaskTemplate, StrategicTaskTemplateId,
};
pub use validation::{validate_strategic_task_report, StrategicTaskValidationError};
