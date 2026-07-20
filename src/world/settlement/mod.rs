//! Settlement treasuries (ADR-093 I7), SettlementState (SA1 / ADR-116),
//! Need Evaluation (SA2 / ADR-117), Response Engine (SA3 / ADR-118),
//! Response Arbiter (SA4 / ADR-119), Building Intent Propagation (SA5 / ADR-120),
//! Strategic Task Generation (SA6 / ADR-121), Emergency Reweighting (SA8 / ADR-123),
//! and Strategic Construction Planning (SA9 / ADR-124).

mod access;
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

pub use access::{
    TreasuryAccessPolicy, TreasuryAccessResult, building_supports_settlement_treasury,
    can_unit_deposit_to_treasury, settlement_interaction_position, settlement_interaction_space,
};
pub use authoring::{CreateSettlementReport, create_settlement_with_treasury};
pub use deposit::{DepositGoldReport, deposit_gold};
pub use error::TreasuryError;
pub use id::{SettlementId, TreasuryId};
pub use record::{
    SettlementOwnership, SettlementRecord, SettlementTreasuryRecord, TreasuryTransactionRecord,
};
pub use membership::reconcile_settlement_building_membership;
pub use store::SettlementStore;
pub use planner::{
    BuildingLocalRetention, ItemDemandEntry, PlannerBuildingDecision, PlannerDiagnostics,
    PlannerShortageKind, PlannerValidationError, ProductionPlannerSaveState,
    ProductionPlannerStore, ProductionPriorityCategory, SettlementProductionPlanner, StockGoal,
    aggregate_settlement_stock, execute_settlement_replan, mark_settlement_planner_dirty,
    replan_settlement_production, step_settlement_production_planners, validate_planner_config,
};
pub use state::{
    ActiveEmergencyInstance, NeedCategory, NeedTarget, SettlementEmergencyState, SettlementKind,
    SettlementModifier, SettlementModifierSource, SettlementPlannerLifecycle, SettlementPolicies,
    SettlementState, SettlementStateSaveState, SettlementStateStore, SettlementStateValidationError,
    default_need_targets_for_kind, ensure_settlement_states_for_world,
    mark_all_settlement_states_dirty, mark_settlement_state_dirty,
    mark_settlement_state_dirty_for_building, validate_settlement_state, validate_settlement_states,
    validate_world_settlement_states,
};
pub use emergency::{
    active_definitions, emergency_blocks_response, emergency_bump_task_priority,
    emergency_need_pressure_delta, emergency_only_gate, emergency_preempt_relaxation,
    emergency_response_score_delta, emergency_unlocks_response, evaluate_settlement_emergencies,
    evaluate_settlement_emergencies_now, starter_emergency_definitions,
    step_settlement_emergency_evaluation, validate_emergency_catalog, validate_emergency_definition,
    EmergencyCatalog, EmergencyCatalogError, EmergencyDefinition, EmergencyEvalContext,
    EmergencyEvaluationReport, EmergencyEvaluationStore, EmergencyEvaluatorKind, EmergencyId,
    EmergencyInterruptionPolicy, EmergencyPreemptRelaxation, EmergencySignalDiagnostic,
    EmergencyValidationError, NeedPressureModifier, ResponseScoreModifier, TaskPriorityModifier,
    EMERGENCY_EVAL_CADENCE_TICKS,
};
pub use needs::{
    apply_pressure_modifiers, evaluate_settlement_needs, evaluate_settlement_needs_now,
    normalize_pressure, starter_need_definitions, step_settlement_need_evaluation,
    validate_need_catalog, validate_need_snapshot, validate_settlement_need_evaluation,
    NeedBlockingReason, NeedCatalog, NeedCatalogError, NeedDefinition, NeedEvalContext,
    NeedEvaluationMethod, NeedEvaluationStore, NeedEvaluationValidationError, NeedId,
    NeedMeasurementType, NeedResponseCategory, NeedSnapshot, NeedTargetSource, NeedTrend,
    SettlementNeedEvaluation, NEED_EVAL_CADENCE_TICKS,
};
pub use response::{
    discover_settlement_responses, discover_settlement_responses_now, score_candidate,
    starter_response_definitions, step_settlement_response_discovery,
    validate_candidate, validate_response_catalog_against_needs,
    validate_response_catalog_definitions, validate_response_catalog_definitions_with_needs,
    validate_settlement_response_candidates, CandidateResponse, CapabilityRequirement,
    ExpectedEffect, ResponseAvailability, ResponseBlockingReason, ResponseCandidateStore,
    ResponseCandidateValidationError, ResponseCatalog, ResponseCatalogError, ResponseDefinition,
    ResponseDiscoveryContext, ResponseId, ResponseType, SettlementResponseCandidates,
    RESPONSE_DISCOVERY_CADENCE_TICKS,
};
pub use arbiter::{
    arbitrate_settlement_intent, arbitrate_settlement_intent_now, arbitration_score,
    step_settlement_response_arbitration, validate_intent, validate_settlement_intent_plan,
    ArbitrationContext, IntentId, IntentPersistence, IntentRejectionReason, IntentValidationError,
    RejectedIntentCandidate, SettlementIntent, SettlementIntentPlan, SettlementIntentStore,
    HIGH_PRESSURE_THRESHOLD, INTENT_ARBITRATION_CADENCE_TICKS, MAX_INTENTS_PER_NEED_HIGH,
    MAX_INTENTS_PER_NEED_NORMAL, MAX_SETTLEMENT_INTENTS, MIN_ARBITRATION_SCORE,
};
pub use intent_apply::{
    building_owned_by_intent_propagation, discover_capable_buildings,
    primary_operation_requirement, propagate_building_intent_now,
    propagate_settlement_intent_to_buildings, step_building_intent_propagation,
    validate_propagation_report, BuildingIntentPropagationReport, BuildingIntentPropagationStore,
    BuildingPolicyAssignment, CapableBuilding, HIGH_INTENT_PRIORITY, IgnoredBuilding,
    INTENT_PROPAGATION_CADENCE_TICKS, MAX_BUILDINGS_PER_INTENT_HIGH,
    MAX_BUILDINGS_PER_INTENT_NORMAL, PropagationContext, PropagationValidationError,
};
pub use task_gen::{
    generate_strategic_tasks_for_settlement, generate_strategic_tasks_now, intent_to_task_priority,
    starter_strategic_task_templates, step_settlement_strategic_task_generation,
    validate_strategic_task_report, StrategicTaskCatalogError, StrategicTaskEmission,
    StrategicTaskGenContext, StrategicTaskGenerationReport, StrategicTaskGenerationStore,
    StrategicTaskTemplate, StrategicTaskTemplateCatalog, StrategicTaskTemplateId,
    StrategicTaskValidationError, STRATEGIC_TASK_GEN_CADENCE_TICKS,
};
pub use construction::{
    approve_construction_plan, best_building_candidate, cancel_construction_plan,
    create_plan_from_manual_placement, estimate_capacity_gap, fulfillment_key,
    mark_construction_planning_dirty_from_intents, plan_construction_for_settlement,
    plan_construction_now, search_placement_candidates, select_building_candidates,
    starter_construction_costs, starter_construction_mappings,
    step_settlement_construction_planning, validate_construction_plans,
    validate_world_construction_plans, BuildingCandidateScore, BuildingConstructionCostCatalog,
    BuildingConstructionCostDefinition, CapacityGapEstimate, ConstructionCapabilityKind,
    ConstructionCatalogError, ConstructionMaterialRequirement, ConstructionPlacementCandidate,
    ConstructionPlan, ConstructionPlanId, ConstructionPlanSaveState, ConstructionPlanSource,
    ConstructionPlanStatus, ConstructionPlanStore, ConstructionPlanningContext,
    ConstructionPlanningReport, ConstructionPlanningReportStore, ConstructionResponseCatalog,
    ConstructionResponseMapping, ConstructionValidationError, PlacementSearchBudget,
    PlacementSearchResult, RejectedSiteDiagnostic, CONSTRUCTION_PLANNING_CADENCE_TICKS,
};

#[cfg(test)]
mod tests;
