//! Planner configuration validation (EP9).

use std::collections::HashSet;

use crate::world::ItemDefinitionId;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::operation::OperationCatalog;

use super::graph::{ProductionGraph, detect_production_cycles};
use super::types::{SettlementProductionPlanner, StockGoal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerValidationError {
    UnknownItem { item_id: ItemDefinitionId },
    DuplicateGoal { item_id: ItemDefinitionId },
    NegativeTarget { item_id: ItemDefinitionId },
    InvalidExportThreshold { item_id: ItemDefinitionId },
    CircularRecipe { cycle: Vec<ItemDefinitionId> },
    NoProducers { item_id: ItemDefinitionId },
}

pub fn validate_planner_config(
    planner: &SettlementProductionPlanner,
    operation_catalog: &OperationCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Vec<PlannerValidationError> {
    let mut errors = Vec::new();
    let graph = ProductionGraph::from_catalog(operation_catalog);
    let cycles = detect_production_cycles(&graph);
    for cycle in cycles {
        errors.push(PlannerValidationError::CircularRecipe { cycle });
    }

    let mut seen_goals = HashSet::new();
    for goal in &planner.stock_goals {
        if inventory_ctx.items.get(&goal.item_id).is_none() {
            errors.push(PlannerValidationError::UnknownItem {
                item_id: goal.item_id.clone(),
            });
        }
        if !seen_goals.insert(goal.item_id.clone()) {
            errors.push(PlannerValidationError::DuplicateGoal {
                item_id: goal.item_id.clone(),
            });
        }
        if goal.maintain_quantity == 0 && goal.export_threshold.is_none() {
            // zero maintain is allowed as "don't stock" but negative is invalid — u32 prevents negative
        }
        if let Some(threshold) = goal.export_threshold {
            if threshold < goal.maintain_quantity {
                errors.push(PlannerValidationError::InvalidExportThreshold {
                    item_id: goal.item_id.clone(),
                });
            }
        }
        if graph.producers_for(&goal.item_id).is_empty() && goal.maintain_quantity > 0 {
            // Items with no producers may still be stocked if hauled from outside — warn only when
            // demand requires production.
            if graph.select_producer(&goal.item_id).is_none() {
                errors.push(PlannerValidationError::NoProducers {
                    item_id: goal.item_id.clone(),
                });
            }
        }
    }
    errors
}
