//! Strategic Construction Planning (SA9 / ADR-124).
//!
//! Converts construction SettlementIntent into persistent ConstructionPlan records with
//! capability-based building selection and bounded placement search. Does not assign workers,
//! move materials, or spawn completed buildings.

mod capacity;
mod catalog;
mod evaluate;
mod placement;
mod plan;
mod report;
mod select;
mod starter;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod tests;

pub use catalog::{
    BuildingConstructionCostCatalog, BuildingConstructionCostDefinition, ConstructionCapabilityKind,
    ConstructionCatalogError, ConstructionResponseCatalog, ConstructionResponseMapping,
};
pub use evaluate::{
    approve_construction_plan, cancel_construction_plan, create_plan_from_manual_placement,
    plan_construction_for_settlement, ConstructionPlanningContext,
};
pub use placement::{search_placement_candidates, PlacementSearchBudget, PlacementSearchResult};
pub use plan::{
    ConstructionMaterialRequirement, ConstructionPlacementCandidate, ConstructionPlan,
    ConstructionPlanId, ConstructionPlanSaveState, ConstructionPlanSource, ConstructionPlanStatus,
};
pub use report::{
    BuildingCandidateScore, ConstructionPlanningReport, RejectedSiteDiagnostic,
};
pub use select::{best_building_candidate, select_building_candidates};
pub use starter::{starter_construction_costs, starter_construction_mappings};
pub use step::{
    mark_construction_planning_dirty_from_intents, plan_construction_now,
    step_settlement_construction_planning, CONSTRUCTION_PLANNING_CADENCE_TICKS,
};
pub use store::{ConstructionPlanStore, ConstructionPlanningReportStore};
pub use validation::{
    validate_construction_plans, validate_world_construction_plans, ConstructionValidationError,
};
pub use capacity::{estimate_capacity_gap, fulfillment_key, CapacityGapEstimate};
