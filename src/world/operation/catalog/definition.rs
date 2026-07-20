//! Authoritative operation definition (EP3).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::category::OperationCategory;
use super::definition_id::OperationDefinitionId;
use super::io::{
    OperationInputDefinition, OperationOutputDefinition, OperationPowerRequirementRef,
    OperationSkillRequirementRef, OperationTerrainRequirementRef, OperationToolRequirementRef,
};

/// Immutable authored description of what a building operation does (EP3).
///
/// Does not store runtime progress. Referenced by [`OperationDefinitionId`] at runtime.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct OperationDefinition {
    pub id: OperationDefinitionId,
    pub display_name: String,
    pub description: String,
    pub category: OperationCategory,
    /// Base labor units per tick before worker/efficiency scaling (EP3).
    pub base_labor: u32,
    pub max_workers: u32,
    pub repeatable: bool,
    /// Future hauling/collection gate (EP5).
    pub requires_collection: bool,
    #[serde(default)]
    pub inputs: Vec<OperationInputDefinition>,
    #[serde(default)]
    pub outputs: Vec<OperationOutputDefinition>,
    #[serde(default)]
    pub terrain_requirements: Vec<OperationTerrainRequirementRef>,
    #[serde(default)]
    pub tool_requirements: Vec<OperationToolRequirementRef>,
    #[serde(default)]
    pub power_requirements: Vec<OperationPowerRequirementRef>,
    #[serde(default)]
    pub skill_requirements: Vec<OperationSkillRequirementRef>,
    pub enabled: bool,
}

impl OperationDefinition {
    pub fn new(
        id: OperationDefinitionId,
        display_name: impl Into<String>,
        description: impl Into<String>,
        category: OperationCategory,
        base_labor: u32,
        max_workers: u32,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            description: description.into(),
            category,
            base_labor,
            max_workers,
            repeatable: true,
            requires_collection: false,
            inputs: Vec::new(),
            outputs: Vec::new(),
            terrain_requirements: Vec::new(),
            tool_requirements: Vec::new(),
            power_requirements: Vec::new(),
            skill_requirements: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_repeatable(mut self, repeatable: bool) -> Self {
        self.repeatable = repeatable;
        self
    }

    pub fn with_requires_collection(mut self, requires_collection: bool) -> Self {
        self.requires_collection = requires_collection;
        self
    }

    pub fn with_inputs(mut self, inputs: Vec<OperationInputDefinition>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn with_outputs(mut self, outputs: Vec<OperationOutputDefinition>) -> Self {
        self.outputs = outputs;
        self
    }

    pub fn with_terrain_requirements(
        mut self,
        terrain_requirements: Vec<OperationTerrainRequirementRef>,
    ) -> Self {
        self.terrain_requirements = terrain_requirements;
        self
    }
}
