//! Authored ResponseDefinition catalog entries (SA3).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::id::ResponseId;
use crate::world::settlement::needs::NeedId;

/// Generic response kind. No kind executes work in SA3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseType {
    IncreaseProduction,
    DecreaseProduction,
    ConstructBuilding,
    RepairBuilding,
    Research,
    Recruit,
    Trade,
    Defend,
    Expand,
}

impl ResponseType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IncreaseProduction => "increase_production",
            Self::DecreaseProduction => "decrease_production",
            Self::ConstructBuilding => "construct_building",
            Self::RepairBuilding => "repair_building",
            Self::Research => "research",
            Self::Recruit => "recruit",
            Self::Trade => "trade",
            Self::Defend => "defend",
            Self::Expand => "expand",
        }
    }
}

/// Authored expected effect — abstract units for scoring only.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ExpectedEffect {
    /// How strongly this response is expected to relieve matching need pressure (0.0..=1.0).
    pub pressure_relief: f32,
    /// Abstract cost units for scoring (higher = less attractive when equal relief).
    pub estimated_cost: f32,
}

impl ExpectedEffect {
    pub fn new(pressure_relief: f32, estimated_cost: f32) -> Self {
        Self {
            pressure_relief,
            estimated_cost,
        }
    }
}

/// Capability gate checked during discovery. Validated against catalogs / world readouts.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRequirement {
    /// Settlement must own a complete building whose definition supports this operation id.
    SupportingOperation(String),
    /// Settlement must own a complete building with this definition id.
    BuildingDefinition(String),
    /// Settlement policies must allow expansion.
    ExpansionEnabled,
    /// Settlement policies must allow automation / planner-driven production.
    AutomationEnabled,
    /// Settlement aggression policy must be at least this value.
    MinAggression(u8),
    /// Always satisfied — used for stubs (trade/recruit) until richer sensors exist.
    Always,
}

/// Authored response definition — content, not runtime state.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ResponseDefinition {
    pub id: ResponseId,
    pub display_name: String,
    pub description: String,
    /// Needs this response may address. Discovery is driven by this list, never need→response code.
    pub supported_need_ids: Vec<NeedId>,
    pub response_type: ResponseType,
    pub expected_effect: ExpectedEffect,
    /// Additive priority modifier applied during scoring.
    pub priority_modifier: i16,
    pub capability_requirements: Vec<CapabilityRequirement>,
    /// Optional prerequisite responses (validated for cycles; not executed in SA3).
    pub prerequisite_response_ids: Vec<ResponseId>,
    /// Future AI metadata seam.
    pub ai_tags: Vec<String>,
    pub enabled: bool,
}

impl ResponseDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        supported_need_ids: impl IntoIterator<Item = NeedId>,
        response_type: ResponseType,
        expected_effect: ExpectedEffect,
        priority_modifier: i16,
        capability_requirements: impl IntoIterator<Item = CapabilityRequirement>,
    ) -> Self {
        Self {
            id: ResponseId::new(id),
            display_name: display_name.into(),
            description: description.into(),
            supported_need_ids: supported_need_ids.into_iter().collect(),
            response_type,
            expected_effect,
            priority_modifier,
            capability_requirements: capability_requirements.into_iter().collect(),
            prerequisite_response_ids: Vec::new(),
            ai_tags: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_prerequisites(
        mut self,
        prerequisites: impl IntoIterator<Item = ResponseId>,
    ) -> Self {
        self.prerequisite_response_ids = prerequisites.into_iter().collect();
        self
    }

    pub fn with_ai_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.ai_tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn supports_need(&self, need_id: &NeedId) -> bool {
        self.supported_need_ids.iter().any(|id| id == need_id)
    }
}
