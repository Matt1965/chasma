//! Settlement production planner module (EP9).

mod apply;
mod graph;
mod inventory;
mod plan;
mod step;
mod store;
mod types;
mod validation;

#[cfg(test)]
mod tests;

pub use apply::{apply_planner_decisions, disable_unselected_planner_buildings};
pub use graph::{ProductionGraph, ProducerRecipe, detect_production_cycles, propagate_demand};
pub use inventory::{aggregate_settlement_stock, count_binding_stock};
pub use plan::{execute_settlement_replan, replan_settlement_production};
pub use step::{mark_settlement_planner_dirty, step_settlement_production_planners};
pub use store::ProductionPlannerStore;
pub use types::{
    BuildingLocalRetention, ItemDemandEntry, PlannerBuildingDecision, PlannerDiagnostics,
    PlannerShortageKind, ProductionPlannerSaveState, ProductionPriorityCategory,
    SettlementProductionPlanner, StockGoal,
};
pub use validation::{PlannerValidationError, validate_planner_config};
