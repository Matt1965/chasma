//! Authored NeedDefinition catalog entries (SA2).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::id::NeedId;
use crate::world::settlement::state::NeedCategory;
use crate::world::settlement::planner::ProductionPriorityCategory;

/// How a need measures its current value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedMeasurementType {
    /// Sum of matching inventory stock units.
    InventoryStock,
    /// Count of buildings matching a lifecycle/role filter.
    BuildingCount,
    /// Count of settlement-affiliated units (workers/population proxy).
    UnitCount,
    /// Policy-derived scalar (e.g. aggression band).
    PolicyScalar,
    /// Explicit stub until a richer sensor exists.
    Stub,
}

/// Where the desired/target value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedTargetSource {
    /// Read `SettlementState.need_targets` for `target_category`.
    SettlementNeedTarget,
    /// Use `default_desired` from the definition when no authored target exists.
    DefinitionDefault,
}

/// Which evaluator implementation runs for this need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedEvaluationMethod {
    FoodStock,
    ConstructionSites,
    HousingCapacity,
    DefensePosture,
    ResearchStub,
    ExpansionGrowth,
    LuxuryStock,
}

/// Future Response category seam (SA3+). Not used for actions in SA2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedResponseCategory {
    Production,
    Construction,
    Defense,
    Research,
    Expansion,
    Luxury,
    None,
}

/// Authored need definition — content, not runtime state.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NeedDefinition {
    pub id: NeedId,
    pub display_name: String,
    pub description: String,
    pub measurement_type: NeedMeasurementType,
    pub target_source: NeedTargetSource,
    /// Maps to `SettlementState.need_targets` category when using SettlementNeedTarget.
    pub target_category: NeedCategory,
    pub evaluation_method: NeedEvaluationMethod,
    pub priority_category: ProductionPriorityCategory,
    pub response_category: NeedResponseCategory,
    /// Fallback desired value when no authored target exists.
    pub default_desired: u32,
    pub enabled: bool,
}

impl NeedDefinition {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        measurement_type: NeedMeasurementType,
        target_source: NeedTargetSource,
        target_category: NeedCategory,
        evaluation_method: NeedEvaluationMethod,
        priority_category: ProductionPriorityCategory,
        response_category: NeedResponseCategory,
        default_desired: u32,
    ) -> Self {
        Self {
            id: NeedId::new(id),
            display_name: display_name.into(),
            description: description.into(),
            measurement_type,
            target_source,
            target_category,
            evaluation_method,
            priority_category,
            response_category,
            default_desired,
            enabled: true,
        }
    }
}
