//! SettlementIntent → BuildingOperationPolicy propagation (SA5).
//!
//! Policy-only. Never touches BuildingOperationState, tasks, logistics, or construction.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::{
    BuildingOperationPolicy, ControlSource, RepeatMode,
};
use crate::world::operation::{OperationCatalog, validate_operation_selection};
use crate::world::settlement::arbiter::SettlementIntentPlan;
use crate::world::settlement::response::{ResponseCatalog, ResponseType};
use crate::world::settlement::SettlementId;
use crate::world::{BuildingId, WorldData};

use super::discover::{discover_capable_buildings, primary_operation_requirement};
use super::report::{
    BuildingIntentPropagationReport, BuildingPolicyAssignment, IgnoredBuilding,
};

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
    };

    // Process highest-priority intents first so they win building slots on conflict.
    let mut intents = ctx.intent_plan.intents.clone();
    intents.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.intent_id.as_str().cmp(b.intent_id.as_str()))
    });

    let mut claimed: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();

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
                apply_enable_intent(ctx, intent, definition, &mut report, &mut claimed, true);
            }
            ResponseType::DecreaseProduction => {
                apply_enable_intent(ctx, intent, definition, &mut report, &mut claimed, false);
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

    report.diagnostics.push(format!(
        "assignments={} ignored={} deferred={}",
        report.assignments.len(),
        report.ignored_buildings.len(),
        report.deferred_intents.len()
    ));
    report
}

fn apply_enable_intent(
    ctx: &mut PropagationContext<'_>,
    intent: &crate::world::settlement::arbiter::SettlementIntent,
    definition: &crate::world::settlement::response::ResponseDefinition,
    report: &mut BuildingIntentPropagationReport,
    claimed: &mut std::collections::BTreeSet<u64>,
    enable: bool,
) {
    let Some(operation_id) = primary_operation_requirement(definition) else {
        report.diagnostics.push(format!(
            "response `{}` has no SupportingOperation capability — cannot propagate",
            definition.id.as_str()
        ));
        return;
    };

    let capable = discover_capable_buildings(
        ctx.world,
        ctx.building_catalog,
        ctx.settlement_id,
        &operation_id,
    );

    if capable.is_empty() {
        report.diagnostics.push(format!(
            "intent {} response `{}`: no capable buildings for operation `{}`",
            intent.intent_id.as_str(),
            intent.chosen_response.as_str(),
            operation_id.as_str()
        ));
        return;
    }

    let max_select = if intent.priority >= HIGH_INTENT_PRIORITY {
        MAX_BUILDINGS_PER_INTENT_HIGH
    } else {
        MAX_BUILDINGS_PER_INTENT_NORMAL
    };

    let mut selected = 0usize;
    for candidate in &capable {
        if claimed.contains(&candidate.building_id.raw()) {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: candidate.building_id,
                response_id: intent.chosen_response.clone(),
                reason: "already claimed by higher-priority intent".into(),
            });
            continue;
        }

        if selected >= max_select {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: candidate.building_id,
                response_id: intent.chosen_response.clone(),
                reason: format!("distribution limit ({max_select}) reached"),
            });
            continue;
        }

        let Some(record) = ctx.world.get_building(candidate.building_id) else {
            report.diagnostics.push(format!(
                "unknown building #{}",
                candidate.building_id.raw()
            ));
            continue;
        };
        let Some(building_def) = ctx.building_catalog.get(&record.definition_id) else {
            report.diagnostics.push(format!(
                "missing building definition for #{}",
                candidate.building_id.raw()
            ));
            continue;
        };
        if validate_operation_selection(
            building_def,
            candidate.building_id,
            ctx.operation_catalog,
            &operation_id,
        )
        .is_err()
        {
            report.ignored_buildings.push(IgnoredBuilding {
                building_id: candidate.building_id,
                response_id: intent.chosen_response.clone(),
                reason: format!("invalid operation `{}`", operation_id.as_str()),
            });
            continue;
        }

        // Snapshot state before policy mutation to prove we don't touch it.
        let state_before = ctx
            .world
            .building_production_store()
            .get_state(candidate.building_id)
            .cloned();

        let priority = intent_priority_to_policy(intent.priority);
        {
            let store = ctx.world.building_production_store_mut();
            store.ensure_policy_for_building(
                candidate.building_id,
                building_def,
                ctx.operation_catalog,
            );
            let policy = store.get_policy_mut(candidate.building_id);
            apply_intent_to_policy(policy, &operation_id, enable, priority);
        }

        let state_after = ctx
            .world
            .building_production_store()
            .get_state(candidate.building_id)
            .cloned();
        if state_before != state_after {
            report.diagnostics.push(format!(
                "ERROR: BuildingOperationState changed for #{} — propagation must be policy-only",
                candidate.building_id.raw()
            ));
        }

        claimed.insert(candidate.building_id.raw());
        selected += 1;
        report.assignments.push(BuildingPolicyAssignment {
            building_id: candidate.building_id,
            intent_id: intent.intent_id.clone(),
            response_id: intent.chosen_response.clone(),
            need_id: intent.source_need.clone(),
            selected_operation: Some(operation_id.clone()),
            enabled: enable,
            priority,
            reason: format!(
                "capability op=`{}` intent_pri={:.1} enable={enable}",
                operation_id.as_str(),
                intent.priority
            ),
        });
    }
}

fn apply_intent_to_policy(
    policy: &mut BuildingOperationPolicy,
    operation_id: &crate::world::operation::OperationDefinitionId,
    enable: bool,
    priority: u8,
) {
    policy.planner_managed = true;
    policy.control_source = ControlSource::AIControlled;
    policy.enabled = enable;
    policy.paused = false;
    policy.selected_operation = Some(operation_id.clone());
    policy.repeat_mode = RepeatMode::Continuous;
    policy.priority = priority;
}

fn intent_priority_to_policy(intent_priority: f32) -> u8 {
    if !intent_priority.is_finite() {
        return 128;
    }
    (intent_priority / 4.0).clamp(32.0, 255.0).round() as u8
}

/// Whether EP9 should leave this building's policy alone.
pub fn building_owned_by_intent_propagation(
    world: &WorldData,
    building_id: BuildingId,
) -> bool {
    world
        .building_intent_propagation_store()
        .is_building_assigned(building_id)
}
