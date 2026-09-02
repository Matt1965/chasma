//! SettlementIntent → BuildingOperationPolicy propagation (SA5).
//!
//! Policy-only. Never touches BuildingOperationState, tasks, logistics, or construction.
//! EP9 recommends production paths; SA5 is the sole AI policy writer (ADR-120 Phase 8).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::operation::{OperationCatalog, validate_operation_selection};
use crate::world::settlement::SettlementId;
use crate::world::settlement::arbiter::SettlementIntentPlan;
use crate::world::settlement::planner::{
    ProductionIntentRequest, SettlementProductionPlanner, demand_quantity_from_need_snapshot,
    priority_category_for_need, recommend_production_for_intent,
};
use crate::world::settlement::response::{ResponseCatalog, ResponseType};
use crate::world::{BuildingId, WorldData};

use super::discover::{discover_capable_buildings, primary_operation_requirement};
use super::policy::{
    can_sa5_mutate_policy, sa5_apply_policy_decision, sa5_disable_unselected_ai_buildings,
};
use super::report::{BuildingIntentPropagationReport, BuildingPolicyAssignment, IgnoredBuilding};

/// Max buildings enabled for a high-priority intent.
pub const MAX_BUILDINGS_PER_INTENT_HIGH: usize = 2;
/// Max buildings enabled for a normal-priority intent.
pub const MAX_BUILDINGS_PER_INTENT_NORMAL: usize = 1;
pub const HIGH_INTENT_PRIORITY: f32 = 100.0;

pub struct PropagationContext<'a> {
    pub world: &'a mut WorldData,
    pub building_catalog: &'a BuildingCatalog,
    pub operation_catalog: &'a OperationCatalog,
    pub response_catalog: &'a ResponseCatalog,
    pub inventory_ctx: &'a InventoryCatalogCtx<'a>,
    pub settlement_id: SettlementId,
    pub intent_plan: &'a SettlementIntentPlan,
    pub simulation_tick: u64,
}

/// Propagate SettlementIntent into BuildingOperationPolicy changes.
pub fn propagate_settlement_intent_to_buildings(
    ctx: &mut PropagationContext<'_>,
) -> BuildingIntentPropagationReport {
    let mut report = BuildingIntentPropagationReport {
        settlement_id: ctx.settlement_id,
        propagated_tick: ctx.simulation_tick,
        source_intent_tick: ctx.intent_plan.planned_tick,
        assignments: Vec::new(),
        ignored_buildings: Vec::new(),
        deferred_intents: Vec::new(),
        diagnostics: Vec::new(),
        planner_diagnostics: Vec::new(),
    };

    let planner = ctx
        .world
        .production_planner_store()
        .get(ctx.settlement_id)
        .cloned()
        .unwrap_or_default();

    let mut intents = ctx.intent_plan.intents.clone();
    intents.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.intent_id.as_str().cmp(b.intent_id.as_str()))
    });

    let mut claimed: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    let mut active_buildings: Vec<BuildingId> = Vec::new();

    for intent in &intents {
        let Some(definition) = ctx.response_catalog.get(&intent.chosen_response) else {
            report.diagnostics.push(format!(
                "unknown response `{}` for intent {}",
                intent.chosen_response.as_str(),
                intent.intent_id.as_str()
            ));
            continue;
        };

        match intent.response_type {
            ResponseType::IncreaseProduction | ResponseType::Research => {
                apply_production_intent(
                    ctx,
                    intent,
                    definition,
                    &planner,
                    &mut report,
                    &mut claimed,
                    &mut active_buildings,
                    true,
                );
            }
            ResponseType::DecreaseProduction => {
                apply_production_intent(
                    ctx,
                    intent,
                    definition,
                    &planner,
                    &mut report,
                    &mut claimed,
                    &mut active_buildings,
                    false,
                );
            }
            ResponseType::ConstructBuilding
            | ResponseType::RepairBuilding
            | ResponseType::Trade
            | ResponseType::Defend
            | ResponseType::Expand
            | ResponseType::Recruit => {
                report.deferred_intents.push(format!(
                    "{} ({}) deferred from SA5 — construction handled by SA9; logistics/tasks by SA6+",
                    intent.chosen_response.as_str(),
                    intent.response_type.as_str()
                ));
            }
        }
    }

    let settlement_buildings = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id);
    sa5_disable_unselected_ai_buildings(ctx.world, &settlement_buildings, &active_buildings);

    report.diagnostics.push(format!(
        "assignments={} ignored={} deferred={} active_buildings={}",
        report.assignments.len(),
        report.ignored_buildings.len(),
        report.deferred_intents.len(),
        active_buildings.len()
    ));
    report
}

fn apply_production_intent(
    ctx: &mut PropagationContext<'_>,
    intent: &crate::world::settlement::arbiter::SettlementIntent,
    definition: &crate::world::settlement::response::ResponseDefinition,
    planner: &SettlementProductionPlanner,
    report: &mut BuildingIntentPropagationReport,
    claimed: &mut std::collections::BTreeSet<u64>,
    active_buildings: &mut Vec<BuildingId>,
    enable: bool,
) {
    let Some(operation_id) = primary_operation_requirement(definition) else {
        report.diagnostics.push(format!(
            "response `{}` has no SupportingOperation capability — cannot propagate",
            definition.id.as_str()
        ));
        return;
    };

    let demand_quantity =
        demand_quantity_from_need_snapshot(ctx.world, ctx.settlement_id, &intent.source_need);
    let request = ProductionIntentRequest {
        settlement_id: ctx.settlement_id,
        need_id: intent.source_need.clone(),
        operation_hint: operation_id.clone(),
        demand_quantity,
        enable,
        priority_category: priority_category_for_need(&intent.source_need),
        reason: format!(
            "intent {} response `{}`",
            intent.intent_id.as_str(),
            intent.chosen_response.as_str()
        ),
    };

    let (mut recommendations, planner_diag) = recommend_production_for_intent(
        ctx.world,
        ctx.building_catalog,
        ctx.operation_catalog,
        ctx.inventory_ctx,
        planner,
        &request,
        ctx.simulation_tick,
    );
    report.planner_diagnostics.push(planner_diag);

    if recommendations.is_empty() && enable {
        // Capability fallback for direct single-stage producers when graph returns nothing.
        for capable in discover_capable_buildings(
            ctx.world,
            ctx.building_catalog,
            ctx.settlement_id,
            &operation_id,
        ) {
            recommendations.push(crate::world::settlement::planner::PlannerBuildingDecision {
                building_id: capable.building_id,
                operation_id: capable.operation_id,
                enabled: true,
                priority: intent_priority_to_policy(intent.priority),
                reason: format!(
                    "capability fallback for operation `{}`",
                    operation_id.as_str()
                ),
            });
        }
    }

    let max_select = if intent.priority >= HIGH_INTENT_PRIORITY {
        MAX_BUILDINGS_PER_INTENT_HIGH
    } else {
        MAX_BUILDINGS_PER_INTENT_NORMAL
    };

    let mut selected = 0usize;
    for decision in recommendations {
        if claimed.contains(&decision.building_id.raw()) {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: decision.building_id,
                response_id: intent.chosen_response.clone(),
                reason: "already claimed by higher-priority intent".into(),
            });
            continue;
        }

        if selected >= max_select && enable {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: decision.building_id,
                response_id: intent.chosen_response.clone(),
                reason: format!("distribution limit ({max_select}) reached"),
            });
            continue;
        }

        let Some(record) = ctx.world.get_building(decision.building_id) else {
            report
                .diagnostics
                .push(format!("unknown building #{}", decision.building_id.raw()));
            continue;
        };
        let Some(building_def) = ctx.building_catalog.get(&record.definition_id) else {
            report.diagnostics.push(format!(
                "missing building definition for #{}",
                decision.building_id.raw()
            ));
            continue;
        };
        if validate_operation_selection(
            building_def,
            decision.building_id,
            ctx.operation_catalog,
            &decision.operation_id,
        )
        .is_err()
        {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: decision.building_id,
                response_id: intent.chosen_response.clone(),
                reason: format!("invalid operation `{}`", decision.operation_id.as_str()),
            });
            continue;
        }

        let policy_before = ctx
            .world
            .building_production_store()
            .get_policy(decision.building_id)
            .cloned()
            .unwrap_or_default();
        if !can_sa5_mutate_policy(&policy_before) {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: decision.building_id,
                response_id: intent.chosen_response.clone(),
                reason: "player-controlled building".into(),
            });
            continue;
        }

        let state_before = ctx
            .world
            .building_production_store()
            .get_state(decision.building_id)
            .cloned();

        if !sa5_apply_policy_decision(
            ctx.world,
            ctx.building_catalog,
            ctx.operation_catalog,
            &decision,
        ) {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: decision.building_id,
                response_id: intent.chosen_response.clone(),
                reason: "policy apply rejected".into(),
            });
            continue;
        }

        let state_after = ctx
            .world
            .building_production_store()
            .get_state(decision.building_id)
            .cloned();
        if state_before != state_after {
            report.diagnostics.push(format!(
                "ERROR: BuildingOperationState changed for #{} — propagation must be policy-only",
                decision.building_id.raw()
            ));
        }

        if decision.enabled {
            claimed.insert(decision.building_id.raw());
            active_buildings.push(decision.building_id);
            selected += 1;
        }

        report.assignments.push(BuildingPolicyAssignment {
            building_id: decision.building_id,
            intent_id: intent.intent_id.clone(),
            response_id: intent.chosen_response.clone(),
            need_id: intent.source_need.clone(),
            selected_operation: Some(decision.operation_id.clone()),
            enabled: decision.enabled,
            priority: decision.priority,
            reason: decision.reason.clone(),
        });
    }
}

fn intent_priority_to_policy(intent_priority: f32) -> u8 {
    if !intent_priority.is_finite() {
        return 128;
    }
    (intent_priority / 4.0).clamp(32.0, 255.0).round() as u8
}
