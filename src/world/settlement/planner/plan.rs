//! Settlement production planning logic (EP9).

use std::collections::{HashMap, HashSet};

use crate::world::ItemDefinitionId;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::ControlSource;
use crate::world::building::operation::OperationDefinitionId;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::is_building_operational;
use crate::world::operation::OperationCatalog;
use crate::world::settlement::SettlementId;
use crate::world::{BuildingId, WorldData};

use super::apply::{apply_planner_decisions, disable_unselected_planner_buildings};
use super::graph::{ProductionGraph, detect_production_cycles, propagate_demand};
use super::inventory::aggregate_settlement_stock;
use super::types::{
    ItemDemandEntry, PlannerBuildingDecision, PlannerDiagnostics, PlannerShortageKind,
    SettlementProductionPlanner,
};
use super::validation::{PlannerValidationError, validate_planner_config};

const MAX_DEMAND_DEPTH: usize = 32;

/// Replan production intent for one settlement (EP9).
pub fn replan_settlement_production(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    settlement_id: SettlementId,
    planner: &SettlementProductionPlanner,
    simulation_tick: u64,
) -> (Vec<PlannerBuildingDecision>, PlannerDiagnostics) {
    let mut diagnostics = PlannerDiagnostics {
        settlement_id: Some(settlement_id),
        plan_tick: simulation_tick,
        ..Default::default()
    };

    let validation_errors = validate_planner_config(planner, operation_catalog, inventory_ctx);
    for error in &validation_errors {
        diagnostics
            .validation_errors
            .push(format_validation_error(error));
        if let PlannerValidationError::CircularRecipe { cycle } = error {
            diagnostics
                .blocked_chains
                .push(format!("Circular recipe: {}", cycle_display(cycle)));
        }
    }

    let graph = ProductionGraph::from_catalog(operation_catalog);
    diagnostics.graph_edges = graph.edges.clone();
    for cycle in detect_production_cycles(&graph) {
        diagnostics
            .blocked_chains
            .push(format!("Circular recipe: {}", cycle_display(&cycle)));
    }

    if !planner.enabled {
        return (Vec::new(), diagnostics);
    }

    let has_blocking_validation = validation_errors.iter().any(|error| {
        matches!(
            error,
            PlannerValidationError::CircularRecipe { .. }
                | PlannerValidationError::UnknownItem { .. }
                | PlannerValidationError::DuplicateGoal { .. }
                | PlannerValidationError::NegativeTarget { .. }
                | PlannerValidationError::InvalidExportThreshold { .. }
        )
    });
    if has_blocking_validation {
        return (Vec::new(), diagnostics);
    }

    let current_stock = aggregate_settlement_stock(
        world,
        building_catalog,
        settlement_id,
        &planner.local_retentions,
        inventory_ctx,
    );

    let mut propagated_demand: HashMap<ItemDefinitionId, u32> = HashMap::new();
    for goal in &planner.stock_goals {
        let current = current_stock.get(&goal.item_id).copied().unwrap_or(0);
        let desired = goal.maintain_quantity;
        let demand = desired.saturating_sub(current);
        let priority = planner.priority_for_category(goal.priority_category);
        diagnostics.stock_entries.push(ItemDemandEntry {
            item_id: goal.item_id.clone(),
            current_stock: current,
            desired_stock: desired,
            demand,
            priority,
        });
        if demand == 0 {
            continue;
        }
        if let Err(item_id) = propagate_demand(
            &graph,
            &goal.item_id,
            demand,
            &mut propagated_demand,
            0,
            MAX_DEMAND_DEPTH,
        ) {
            diagnostics.shortages.push((
                item_id,
                PlannerShortageKind::CircularRecipe,
            ));
        }
    }
    diagnostics.propagated_demand = propagated_demand.clone();

    let producers = discover_settlement_producers(world, building_catalog, settlement_id);
    let mut decisions = Vec::new();
    let mut enabled_buildings = HashSet::new();

    let mut demanded_outputs: HashSet<ItemDefinitionId> = propagated_demand
        .iter()
        .filter(|(_, qty)| **qty > 0)
        .map(|(item, _)| item.clone())
        .collect();

    for goal in &planner.stock_goals {
        let current = current_stock.get(&goal.item_id).copied().unwrap_or(0);
        if current >= goal.maintain_quantity {
            demanded_outputs.remove(&goal.item_id);
        }
    }

    for (item_id, demand_qty) in &propagated_demand {
        if *demand_qty == 0 {
            continue;
        }
        if !demanded_outputs.contains(item_id) && !planner.stock_goals.iter().any(|g| {
            g.item_id == *item_id
                && current_stock.get(&g.item_id).copied().unwrap_or(0) < g.maintain_quantity
        }) {
            continue;
        }

        let Some(recipe) =
            select_producer_for_settlement(&graph, &producers, item_id)
        else {
            diagnostics.shortages.push((
                item_id.clone(),
                PlannerShortageKind::NoProducers,
            ));
            continue;
        };

        let mut candidates: Vec<_> = producers
            .iter()
            .filter(|candidate| candidate.operation_id == recipe.operation_id)
            .cloned()
            .collect();
        if candidates.is_empty() {
            diagnostics.shortages.push((
                item_id.clone(),
                PlannerShortageKind::NoOperationalProducers,
            ));
            continue;
        }
        candidates.sort_by(|a, b| b.policy_priority.cmp(&a.policy_priority).then_with(|| {
            a.building_id.raw().cmp(&b.building_id.raw())
        }));

        for candidate in candidates {
            let priority = planner.priority_for_category(recipe.category);
            decisions.push(PlannerBuildingDecision {
                building_id: candidate.building_id,
                operation_id: recipe.operation_id.clone(),
                enabled: true,
                priority,
                reason: format!(
                    "Produce {} (demand {demand_qty}, stock goal)",
                    item_id.as_str()
                ),
            });
            enabled_buildings.insert(candidate.building_id);
        }
    }

    diagnostics.chosen_producers = decisions.clone();
    (decisions, diagnostics)
}

/// Execute replan and apply policy changes for one settlement (EP9).
pub fn execute_settlement_replan(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    settlement_id: SettlementId,
    planner: &mut SettlementProductionPlanner,
    simulation_tick: u64,
) {
    let (decisions, diagnostics) = replan_settlement_production(
        world,
        building_catalog,
        operation_catalog,
        inventory_ctx,
        settlement_id,
        planner,
        simulation_tick,
    );
    planner.last_diagnostics = diagnostics;
    planner.last_plan_tick = simulation_tick;
    planner.dirty = false;

    let settlement_buildings: Vec<BuildingId> = world
        .settlement_store()
        .buildings_for_settlement(settlement_id);
    let active: Vec<BuildingId> = decisions
        .iter()
        .filter(|decision| decision.enabled)
        .map(|decision| decision.building_id)
        .collect();
    disable_unselected_planner_buildings(world, &settlement_buildings, &active);
    apply_planner_decisions(world, building_catalog, operation_catalog, &decisions);
}

#[derive(Debug, Clone)]
struct ProducerCandidate {
    building_id: BuildingId,
    operation_id: OperationDefinitionId,
    policy_priority: u8,
}

fn select_producer_for_settlement(
    graph: &super::graph::ProductionGraph,
    producers: &[ProducerCandidate],
    item_id: &ItemDefinitionId,
) -> Option<super::graph::ProducerRecipe> {
    graph
        .producers_for(item_id)
        .iter()
        .filter(|recipe| {
            producers
                .iter()
                .any(|candidate| candidate.operation_id == recipe.operation_id)
        })
        .max_by_key(|recipe| {
            (
                recipe.category.default_priority(),
                recipe.output_quantity,
                recipe.operation_id.as_str(),
            )
        })
        .cloned()
}

fn discover_settlement_producers(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    settlement_id: SettlementId,
) -> Vec<ProducerCandidate> {
    let mut candidates = Vec::new();
    for building_id in world
        .settlement_store()
        .buildings_for_settlement(settlement_id)
    {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if !is_building_operational(record) {
            continue;
        }
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if definition.supported_operations.is_empty() {
            continue;
        }
        let policy = world
            .building_production_store()
            .get_policy(building_id)
            .cloned()
            .unwrap_or_default();
        if policy.control_source == ControlSource::PlayerControlled && policy.planner_managed {
            // Player reclaimed a previously planner-managed building.
            continue;
        }
        for operation_id in &definition.supported_operations {
            candidates.push(ProducerCandidate {
                building_id,
                operation_id: operation_id.clone(),
                policy_priority: policy.priority,
            });
        }
    }
    candidates
}

fn format_validation_error(error: &PlannerValidationError) -> String {
    match error {
        PlannerValidationError::UnknownItem { item_id } => {
            format!("Unknown item `{}`", item_id.as_str())
        }
        PlannerValidationError::DuplicateGoal { item_id } => {
            format!("Duplicate stock goal for `{}`", item_id.as_str())
        }
        PlannerValidationError::NegativeTarget { item_id } => {
            format!("Negative stock target for `{}`", item_id.as_str())
        }
        PlannerValidationError::InvalidExportThreshold { item_id } => {
            format!("Export threshold below maintain quantity for `{}`", item_id.as_str())
        }
        PlannerValidationError::CircularRecipe { cycle } => {
            format!("Circular recipe: {}", cycle_display(cycle))
        }
        PlannerValidationError::NoProducers { item_id } => {
            format!("No producers for `{}`", item_id.as_str())
        }
    }
}

fn cycle_display(cycle: &[ItemDefinitionId]) -> String {
    cycle
        .iter()
        .map(|item| item.as_str())
        .collect::<Vec<_>>()
        .join(" -> ")
}
