//! Settlement treasuries (ADR-093 I7), SettlementState (SA1 / ADR-116),
//! Need Evaluation (SA2 / ADR-117), Response Engine (SA3 / ADR-118),
//! Response Arbiter (SA4 / ADR-119), Building Intent Propagation (SA5 / ADR-120),
//! Strategic Task Generation (SA6 / ADR-121), Emergency Reweighting (SA8 / ADR-123),
//! and Strategic Construction Planning (SA9 / ADR-124).

mod access;
mod anchor;
mod arbiter;
mod authoring;
pub mod construction;
mod deposit;
pub mod emergency;
mod error;
mod id;
mod intent_apply;
mod membership;
mod needs;
mod planner;
mod record;
mod response;
mod state;
mod store;
mod task_gen;
mod workforce;

pub use access::{
    TreasuryAccessPolicy, TreasuryAccessResult, building_supports_settlement_treasury,
    can_unit_deposit_to_treasury, settlement_interaction_position, settlement_interaction_space,
};
pub use anchor::{
    DEFAULT_TOWN_BOUNDARY_RADIUS_METERS, SETTLEMENT_PLACEMENT_MARGIN_METERS, SettlementAnchorId,
    SettlementAnchorRecord, SettlementAnchorStore, SettlementCreationError,
    initial_boundary_radius_meters, required_center_separation_meters,
    settlement_overlaps_existing,
};
pub use arbiter::{
    ArbitrationContext, HIGH_PRESSURE_THRESHOLD, INTENT_ARBITRATION_CADENCE_TICKS, IntentId,
    IntentPersistence, IntentRejectionReason, IntentValidationError, MAX_INTENTS_PER_NEED_HIGH,
    MAX_INTENTS_PER_NEED_NORMAL, MAX_SETTLEMENT_INTENTS, MIN_ARBITRATION_SCORE,
    RejectedIntentCandidate, SettlementIntent, SettlementIntentPlan, SettlementIntentStore,
    arbitrate_settlement_intent, arbitrate_settlement_intent_now, arbitration_score,
    step_settlement_response_arbitration, validate_intent, validate_settlement_intent_plan,
};
pub use authoring::{CreateSettlementReport, create_settlement, create_settlement_with_treasury};
pub use construction::{
    BuildingCandidateScore, BuildingConstructionCostCatalog, BuildingConstructionCostDefinition,
    CONSTRUCTION_PLANNING_CADENCE_TICKS, CapacityGapEstimate, ConstructionCapabilityKind,
    ConstructionCatalogError, ConstructionMaterialRequirement, ConstructionPlacementCandidate,
    ConstructionPlan, ConstructionPlanId, ConstructionPlanSaveState, ConstructionPlanSource,
    ConstructionPlanStatus, ConstructionPlanStore, ConstructionPlanningContext,
    ConstructionPlanningReport, ConstructionPlanningReportStore, ConstructionResponseCatalog,
    ConstructionResponseMapping, ConstructionValidationError, PlacementSearchBudget,
    PlacementSearchResult, RejectedSiteDiagnostic, approve_construction_plan,
    best_building_candidate, cancel_construction_plan, create_plan_from_manual_placement,
    estimate_capacity_gap, fulfillment_key, mark_construction_planning_dirty_from_intents,
    plan_construction_for_settlement, plan_construction_now, search_placement_candidates,
    select_building_candidates, starter_construction_costs, starter_construction_mappings,
    step_settlement_construction_planning, validate_construction_plans,
    validate_world_construction_plans,
};
pub use deposit::{DepositGoldReport, deposit_gold};
pub use emergency::{
    EMERGENCY_EVAL_CADENCE_TICKS, EmergencyCatalog, EmergencyCatalogError, EmergencyDefinition,
    EmergencyEvalContext, EmergencyEvaluationReport, EmergencyEvaluationStore,
    EmergencyEvaluatorKind, EmergencyId, EmergencyInterruptionPolicy, EmergencyPreemptRelaxation,
    EmergencySignalDiagnostic, EmergencyValidationError, NeedPressureModifier,
    ResponseScoreModifier, TaskPriorityModifier, active_definitions, emergency_blocks_response,
    emergency_bump_task_priority, emergency_need_pressure_delta, emergency_only_gate,
    emergency_preempt_relaxation, emergency_response_score_delta, emergency_unlocks_response,
    evaluate_settlement_emergencies, evaluate_settlement_emergencies_now,
    starter_emergency_definitions, step_settlement_emergency_evaluation,
    validate_emergency_catalog, validate_emergency_definition,
};
pub use error::TreasuryError;
pub use id::{SettlementId, TreasuryId};
pub use intent_apply::{
    BuildingIntentPropagationReport, BuildingIntentPropagationStore, BuildingPolicyAssignment,
    CapableBuilding, HIGH_INTENT_PRIORITY, INTENT_PROPAGATION_CADENCE_TICKS, IgnoredBuilding,
    MAX_BUILDINGS_PER_INTENT_HIGH, MAX_BUILDINGS_PER_INTENT_NORMAL, PropagationContext,
    PropagationValidationError, can_sa5_mutate_policy, discover_capable_buildings,
    primary_operation_requirement, propagate_building_intent_now,
    propagate_settlement_intent_to_buildings, sa5_apply_policy_decision,
    sa5_disable_unselected_ai_buildings, step_building_intent_propagation,
    validate_propagation_report,
};
pub use membership::{
    SettlementMembershipError, assign_building_settlement, assign_selected_units_at_position,
    assign_unit_settlement, clear_unit_settlement_on_removal,
    rebuild_settlement_membership_indexes, seed_building_settlement_at_creation,
    settlement_containing_position,
};
pub use needs::{
    NEED_EVAL_CADENCE_TICKS, NeedBlockingReason, NeedCatalog, NeedCatalogError, NeedDefinition,
    NeedEvalContext, NeedEvaluationMethod, NeedEvaluationStore, NeedEvaluationValidationError,
    NeedId, NeedMeasurementType, NeedResponseCategory, NeedSnapshot, NeedTargetSource, NeedTrend,
    SettlementNeedEvaluation, apply_pressure_modifiers, evaluate_settlement_needs,
    evaluate_settlement_needs_now, normalize_pressure, starter_need_definitions,
    step_settlement_need_evaluation, validate_need_catalog, validate_need_snapshot,
    validate_settlement_need_evaluation,
};
pub use planner::{
    BuildingLocalRetention, ItemDemandEntry, PlannerBuildingDecision, PlannerDiagnostics,
    PlannerShortageKind, PlannerValidationError, ProductionIntentRequest,
    ProductionPlannerSaveState, ProductionPlannerStore, ProductionPriorityCategory,
    SettlementProductionPlanner, StockGoal, aggregate_settlement_stock,
    apply_production_recommendations_for_tests, building_advertises_settlement_supply,
    collect_settlement_accessible_stock, demand_quantity_from_need_snapshot,
    execute_settlement_replan, mark_settlement_planner_dirty, priority_category_for_need,
    recommend_production_for_intent, replan_settlement_production,
    step_settlement_production_planners, sum_category_count, sum_category_nutrition,
    validate_planner_config,
};
pub use record::{
    SettlementOwnership, SettlementRecord, SettlementTreasuryRecord, TreasuryTransactionRecord,
};
pub use response::{
    CandidateResponse, CapabilityRequirement, ExpectedEffect, RESPONSE_DISCOVERY_CADENCE_TICKS,
    ResponseAvailability, ResponseBlockingReason, ResponseCandidateStore,
    ResponseCandidateValidationError, ResponseCatalog, ResponseCatalogError, ResponseDefinition,
    ResponseDiscoveryContext, ResponseId, ResponseType, SettlementResponseCandidates,
    discover_settlement_responses, discover_settlement_responses_now, score_candidate,
    starter_response_definitions, step_settlement_response_discovery, validate_candidate,
    validate_response_catalog_against_needs, validate_response_catalog_definitions,
    validate_response_catalog_definitions_with_needs, validate_settlement_response_candidates,
};
pub use state::{
    ActiveEmergencyInstance, NeedCategory, NeedTarget, SettlementEmergencyState, SettlementKind,
    SettlementModifier, SettlementModifierSource, SettlementPlannerLifecycle, SettlementPolicies,
    SettlementState, SettlementStateSaveState, SettlementStateStore,
    SettlementStateValidationError, default_need_targets_for_kind,
    ensure_settlement_states_for_world, mark_all_settlement_states_dirty,
    mark_settlement_state_dirty, mark_settlement_state_dirty_for_building,
    validate_settlement_state, validate_settlement_states, validate_world_settlement_states,
};
pub use store::SettlementStore;
pub use task_gen::{
    STRATEGIC_TASK_GEN_CADENCE_TICKS, StrategicTaskCatalogError, StrategicTaskEmission,
    StrategicTaskGenContext, StrategicTaskGenerationReport, StrategicTaskGenerationStore,
    StrategicTaskTemplate, StrategicTaskTemplateCatalog, StrategicTaskTemplateId,
    StrategicTaskValidationError, generate_strategic_tasks_for_settlement,
    generate_strategic_tasks_now, intent_to_task_priority, starter_strategic_task_templates,
    step_settlement_strategic_task_generation, validate_strategic_task_report,
};
pub use workforce::{
    WorkPermissionDomain, WorkforcePermissionError, WorkforcePermissionStore,
    clear_settlement_workforce_permissions, clear_unit_workforce_permissions,
    set_unit_work_permission, unit_may_autonomously_perform_work, unit_work_allowed,
    work_permission_domain_for_task,
};

#[cfg(test)]
mod anchor_tests;
#[cfg(test)]
mod tests;
