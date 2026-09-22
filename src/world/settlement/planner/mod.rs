//! Settlement production planner module (EP9).

#[cfg(test)]
mod apply;
mod graph;
mod inventory;
mod plan;
mod service;
mod step;
mod store;
mod types;
mod validation;

#[cfg(test)]
mod tests;

pub use inventory::{
    aggregate_settlement_stock, building_advertises_settlement_supply,
    collect_settlement_accessible_stock, sum_category_count,
    sum_category_nutrition,
};
pub use plan::{execute_settlement_replan, replan_settlement_production};
pub use service::{
    ProductionIntentRequest, demand_quantity_from_need_snapshot, priority_category_for_need,
    recommend_production_for_intent,
};
pub use step::{mark_settlement_planner_dirty, step_settlement_production_planners};
pub use store::ProductionPlannerStore;
pub use types::{
    BuildingLocalRetention, ItemDemandEntry, PlannerBuildingDecision, PlannerDiagnostics,
    PlannerShortageKind, ProductionPlannerSaveState, ProductionPriorityCategory,
    SettlementProductionPlanner, StockGoal,
};
pub use validation::{PlannerValidationError, validate_planner_config};
