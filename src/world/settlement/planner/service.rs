//! EP9 production recommendation service (ADR-114 / ADR-120 Phase 8).
//!
//! Graph reasoning and producer discovery only — never writes BuildingOperationPolicy.

use std::collections::{HashMap, HashSet};

use crate::world::ItemDefinitionId;
use crate::world::WorldData;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::{ControlSource, OperationDefinitionId};
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::operation::{OperationCatalog, OperationOutputDefinition};
use crate::world::settlement::SettlementId;
use crate::world::settlement::needs::NeedId;

use super::graph::{ProductionGraph, propagate_demand};
use super::inventory::aggregate_settlement_stock;
use super::plan::{discover_settlement_producers, select_producer_for_settlement};
use super::types::{
    ItemDemandEntry, PlannerBuildingDecision, PlannerDiagnostics, PlannerShortageKind,
    ProductionPriorityCategory, SettlementProductionPlanner,
};

const MAX_DEMAND_DEPTH: usize = 32;

/// Intent-driven production recommendation request (demand authority = SettlementIntent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionIntentRequest {
    pub settlement_id: SettlementId,
    pub need_id: NeedId,
    pub operation_hint: OperationDefinitionId,
    pub demand_quantity: u32,
    pub enable: bool,
    pub priority_category: ProductionPriorityCategory,
    pub reason: String,
}

/// Recommend which existing settlement producers should participate for one intent.
pub fn recommend_production_for_intent(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    planner: &SettlementProductionPlanner,
    request: &ProductionIntentRequest,
    simulation_tick: u64,
) -> (Vec<PlannerBuildingDecision>, PlannerDiagnostics) {
    let mut diagnostics = PlannerDiagnostics {
        settlement_id: Some(request.settlement_id),
        plan_tick: simulation_tick,
        ..Default::default()
    };

    if !request.enable {
        let decisions = recommend_disable_for_operation(
            world,
            building_catalog,
            &request.operation_hint,
            &request.reason,
        );
        diagnostics.chosen_producers = decisions.clone();
        return (decisions, diagnostics);
    }

    let graph = ProductionGraph::from_catalog(operation_catalog);
    let producers = discover_settlement_producers(world, building_catalog, request.settlement_id);

    let output_items = operation_output_items(operation_catalog, &request.operation_hint);
    if output_items.is_empty() {
        diagnostics.validation_errors.push(format!(
            "operation `{}` has no item outputs for graph planning",
            request.operation_hint.as_str()
        ));
        return (Vec::new(), diagnostics);
    }

    let current_stock = aggregate_settlement_stock(
        world,
        building_catalog,
        request.settlement_id,
        &planner.local_retentions,
        inventory_ctx,
    );

    let mut propagated_demand: HashMap<ItemDefinitionId, u32> = HashMap::new();
    for item_id in &output_items {
        let current = current_stock.get(item_id).copied().unwrap_or(0);
        let demand = request.demand_quantity.max(1);
        diagnostics.stock_entries.push(ItemDemandEntry {
            item_id: item_id.clone(),
            current_stock: current,
            desired_stock: current.saturating_add(demand),
            demand,
            priority: planner.priority_for_category(request.priority_category),
        });
        if let Err(item_id) = propagate_demand(
            &graph,
            item_id,
            demand,
            &mut propagated_demand,
            0,
            MAX_DEMAND_DEPTH,
        ) {
            diagnostics
                .shortages
                .push((item_id, PlannerShortageKind::CircularRecipe));
        }
    }
    diagnostics.propagated_demand = propagated_demand.clone();

    let mut decisions = Vec::new();
    let mut enabled_buildings = HashSet::new();

    for (item_id, demand_qty) in &propagated_demand {
        if *demand_qty == 0 {
            continue;
        }
        let Some(recipe) = select_producer_for_settlement(&graph, &producers, item_id) else {
            diagnostics
                .shortages
                .push((item_id.clone(), PlannerShortageKind::NoProducers));
            continue;
        };

        let mut candidates: Vec<_> = producers
            .iter()
            .filter(|candidate| candidate.operation_id == recipe.operation_id)
            .cloned()
            .collect();
        if candidates.is_empty() {
            diagnostics
                .shortages
                .push((item_id.clone(), PlannerShortageKind::NoOperationalProducers));
            continue;
        }
        candidates.sort_by(|a, b| {
            b.policy_priority
                .cmp(&a.policy_priority)
                .then_with(|| a.building_id.raw().cmp(&b.building_id.raw()))
        });

        for candidate in candidates {
            let priority = planner.priority_for_category(recipe.category);
            decisions.push(PlannerBuildingDecision {
                building_id: candidate.building_id,
                operation_id: recipe.operation_id.clone(),
                enabled: true,
                priority,
                reason: format!(
                    "{} (intent need `{}`, demand {demand_qty})",
                    request.reason,
                    request.need_id.as_str()
                ),
            });
            enabled_buildings.insert(candidate.building_id);
        }
    }

    // Ensure the hinted operation's direct producers are included when graph matched the output.
    if decisions.is_empty() {
        for candidate in producers
            .iter()
            .filter(|candidate| candidate.operation_id == request.operation_hint)
        {
            let priority = planner.priority_for_category(request.priority_category);
            decisions.push(PlannerBuildingDecision {
                building_id: candidate.building_id,
                operation_id: request.operation_hint.clone(),
                enabled: true,
                priority,
                reason: request.reason.clone(),
            });
        }
    }

    diagnostics.chosen_producers = decisions.clone();
    (decisions, diagnostics)
}

/// Derive demand quantity from the current need snapshot deficit (intent demand authority).
pub fn demand_quantity_from_need_snapshot(
    world: &WorldData,
    settlement_id: SettlementId,
    need_id: &NeedId,
) -> u32 {
    world
        .need_evaluation_store()
        .get(settlement_id)
        .and_then(|evaluation| evaluation.snapshot(need_id))
        .map(|snapshot| snapshot.deficit.max(1.0).round() as u32)
        .unwrap_or(1)
        .max(1)
}

pub fn priority_category_for_need(need_id: &NeedId) -> ProductionPriorityCategory {
    match need_id.as_str() {
        "food" => ProductionPriorityCategory::Food,
        "materials" | "construction" => ProductionPriorityCategory::Construction,
        "medicine" => ProductionPriorityCategory::Medicine,
        _ => ProductionPriorityCategory::General,
    }
}

fn recommend_disable_for_operation(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_id: &OperationDefinitionId,
    reason: &str,
) -> Vec<PlannerBuildingDecision> {
    let mut decisions = Vec::new();
    for building_id in world.sorted_building_ids() {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if !definition.supports_operation(operation_id) {
            continue;
        }
        let policy = world
            .building_production_store()
            .get_policy(building_id)
            .cloned()
            .unwrap_or_default();
        if policy.control_source == ControlSource::PlayerControlled {
            continue;
        }
        decisions.push(PlannerBuildingDecision {
            building_id,
            operation_id: operation_id.clone(),
            enabled: false,
            priority: policy.priority,
            reason: reason.to_string(),
        });
    }
    decisions
}

fn operation_output_items(
    operation_catalog: &OperationCatalog,
    operation_id: &OperationDefinitionId,
) -> Vec<ItemDefinitionId> {
    let Some(definition) = operation_catalog.get(operation_id) else {
        return Vec::new();
    };
    definition
        .outputs
        .iter()
        .filter_map(|output| {
            if let OperationOutputDefinition::Item { item_id, .. } = output {
                Some(item_id.clone())
            } else {
                None
            }
        })
        .collect()
}
