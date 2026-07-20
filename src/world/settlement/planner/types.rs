//! Settlement production planner data types (EP9).

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::ItemDefinitionId;
use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operation::OperationDefinitionId;
use crate::world::settlement::SettlementId;
use crate::world::BuildingId;

/// Settlement-wide production priority band (EP9).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Reflect, Serialize, Deserialize,
)]
pub enum ProductionPriorityCategory {
    #[default]
    General,
    Luxury,
    Construction,
    Medicine,
    Food,
}

impl ProductionPriorityCategory {
    pub fn default_priority(self) -> u8 {
        match self {
            Self::Food | Self::Medicine => 255,
            Self::Construction => 192,
            Self::General => 128,
            Self::Luxury => 64,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Luxury => "Luxury",
            Self::Construction => "Construction",
            Self::Medicine => "Medicine",
            Self::Food => "Food",
        }
    }
}

/// Authored settlement stock goal — desired inventory level for one item (EP9).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct StockGoal {
    pub item_id: ItemDefinitionId,
    pub maintain_quantity: u32,
    /// Surplus above this quantity is eligible for export (policy only; no trading).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_threshold: Option<u32>,
    #[serde(default)]
    pub priority_category: ProductionPriorityCategory,
}

/// Minimum inventory to retain in a building binding before counting as settlement surplus (EP9).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct BuildingLocalRetention {
    pub building_definition_id: BuildingDefinitionId,
    pub binding_id: BuildingInventoryBindingId,
    pub item_id: ItemDefinitionId,
    pub retain_quantity: u32,
}

/// Per-settlement production planner configuration and runtime bookkeeping (EP9).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct SettlementProductionPlanner {
    pub enabled: bool,
    #[serde(default)]
    pub stock_goals: Vec<StockGoal>,
    #[serde(default)]
    pub category_priorities: BTreeMap<ProductionPriorityCategory, u8>,
    #[serde(default)]
    pub local_retentions: Vec<BuildingLocalRetention>,
    /// Minimum ticks between automatic replans when not dirty.
    #[serde(default = "default_replan_interval")]
    pub replan_interval_ticks: u64,
    #[serde(default)]
    pub last_plan_tick: u64,
    /// Runtime-only: replan requested before next step.
    #[serde(skip)]
    pub dirty: bool,
    /// Runtime-only: last plan diagnostics for dev inspection.
    #[serde(skip)]
    pub last_diagnostics: PlannerDiagnostics,
}

fn default_replan_interval() -> u64 {
    60
}

impl Default for SettlementProductionPlanner {
    fn default() -> Self {
        Self {
            enabled: true,
            stock_goals: Vec::new(),
            category_priorities: default_category_priorities(),
            local_retentions: Vec::new(),
            replan_interval_ticks: default_replan_interval(),
            last_plan_tick: 0,
            dirty: true,
            last_diagnostics: PlannerDiagnostics::default(),
        }
    }
}

impl SettlementProductionPlanner {
    pub fn priority_for_category(&self, category: ProductionPriorityCategory) -> u8 {
        self.category_priorities
            .get(&category)
            .copied()
            .unwrap_or_else(|| category.default_priority())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

fn default_category_priorities() -> BTreeMap<ProductionPriorityCategory, u8> {
    BTreeMap::from([
        (ProductionPriorityCategory::Food, 255),
        (ProductionPriorityCategory::Medicine, 255),
        (ProductionPriorityCategory::Construction, 192),
        (ProductionPriorityCategory::General, 128),
        (ProductionPriorityCategory::Luxury, 64),
    ])
}

/// Why production cannot proceed for one item (EP9).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum PlannerShortageKind {
    NoProducers,
    NoOperationalProducers,
    InputShortage { item_id: ItemDefinitionId },
    CircularRecipe,
    UnknownItem,
}

/// One planner decision applied to a building (EP9).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct PlannerBuildingDecision {
    pub building_id: BuildingId,
    pub operation_id: OperationDefinitionId,
    pub enabled: bool,
    pub priority: u8,
    pub reason: String,
}

/// Derived demand for one item after stock-goal comparison and propagation (EP9).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct ItemDemandEntry {
    pub item_id: ItemDefinitionId,
    pub current_stock: u32,
    pub desired_stock: u32,
    pub demand: u32,
    pub priority: u8,
}

/// Edge in the derived production dependency graph (EP9, not persisted).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct ProductionGraphEdge {
    pub output_item: ItemDefinitionId,
    pub input_item: ItemDefinitionId,
    pub operation_id: OperationDefinitionId,
}

/// Runtime diagnostics from the latest replan (EP9, not persisted).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct PlannerDiagnostics {
    pub settlement_id: Option<SettlementId>,
    pub plan_tick: u64,
    pub stock_entries: Vec<ItemDemandEntry>,
    pub propagated_demand: HashMap<ItemDefinitionId, u32>,
    pub graph_edges: Vec<ProductionGraphEdge>,
    pub chosen_producers: Vec<PlannerBuildingDecision>,
    pub shortages: Vec<(ItemDefinitionId, PlannerShortageKind)>,
    pub blocked_chains: Vec<String>,
    pub validation_errors: Vec<String>,
}

/// Serializable planner state for all settlements (EP9).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProductionPlannerSaveState {
    pub planners: HashMap<u64, SettlementProductionPlanner>,
}
