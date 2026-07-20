//! Authored construction response → capability → building mappings (SA9).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::building::category::BuildingCategoryId;
use crate::world::building::operation::OperationDefinitionId;
use crate::world::item::ItemDefinitionId;
use crate::world::settlement::response::ResponseId;

use super::starter::{starter_construction_costs, starter_construction_mappings};

/// How a response selects eligible buildings (capability-based, not need→building).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum ConstructionCapabilityKind {
    /// Match buildings that support this operation.
    SupportingOperation(OperationDefinitionId),
    /// Match buildings in this category.
    BuildingCategory(BuildingCategoryId),
    /// Explicit allow-list only (still capability-named for diagnostics).
    ExplicitAllowList,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ConstructionResponseMapping {
    pub response_id: ResponseId,
    pub display_name: String,
    pub capability_kind: ConstructionCapabilityKind,
    /// Diagnostic / fulfillment key fragment (e.g. `food_production`).
    pub capability_key: String,
    /// Optional explicit building allow-list. Empty = discover via capability_kind.
    pub eligible_building_ids: Vec<BuildingDefinitionId>,
    /// Desired count of capable buildings (existing + planned) before skipping new plans.
    pub target_capacity: u32,
    /// When false (e.g. advance_construction), never create new capacity.
    pub creates_new_capacity: bool,
    pub enabled: bool,
}

impl ConstructionResponseMapping {
    pub fn new(
        response_id: impl Into<String>,
        display_name: impl Into<String>,
        capability_kind: ConstructionCapabilityKind,
        capability_key: impl Into<String>,
        target_capacity: u32,
        creates_new_capacity: bool,
    ) -> Self {
        Self {
            response_id: ResponseId::new(response_id),
            display_name: display_name.into(),
            capability_kind,
            capability_key: capability_key.into(),
            eligible_building_ids: Vec::new(),
            target_capacity,
            creates_new_capacity,
            enabled: true,
        }
    }

    pub fn with_eligible_buildings(
        mut self,
        ids: impl IntoIterator<Item = BuildingDefinitionId>,
    ) -> Self {
        self.eligible_building_ids = ids.into_iter().collect();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingConstructionCostDefinition {
    pub building_definition_id: BuildingDefinitionId,
    pub materials: Vec<(ItemDefinitionId, u32)>,
}

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct ConstructionResponseCatalog {
    mappings: Vec<ConstructionResponseMapping>,
    by_response: HashMap<ResponseId, usize>,
}

impl Default for ConstructionResponseCatalog {
    fn default() -> Self {
        Self::from_mappings(starter_construction_mappings())
            .expect("starter construction mappings are valid")
    }
}

impl ConstructionResponseCatalog {
    pub fn from_mappings(
        mappings: Vec<ConstructionResponseMapping>,
    ) -> Result<Self, ConstructionCatalogError> {
        let mut by_response = HashMap::new();
        for (index, mapping) in mappings.iter().enumerate() {
            if mapping.response_id.as_str().is_empty() {
                return Err(ConstructionCatalogError::EmptyResponseId);
            }
            if by_response
                .insert(mapping.response_id.clone(), index)
                .is_some()
            {
                return Err(ConstructionCatalogError::DuplicateResponseId(
                    mapping.response_id.clone(),
                ));
            }
        }
        Ok(Self {
            mappings,
            by_response,
        })
    }

    pub fn mappings(&self) -> &[ConstructionResponseMapping] {
        &self.mappings
    }

    pub fn get(&self, response_id: &ResponseId) -> Option<&ConstructionResponseMapping> {
        self.by_response
            .get(response_id)
            .map(|&i| &self.mappings[i])
    }

    pub fn get_str(&self, response_id: &str) -> Option<&ConstructionResponseMapping> {
        self.get(&ResponseId::new(response_id))
    }
}

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingConstructionCostCatalog {
    costs: HashMap<BuildingDefinitionId, BuildingConstructionCostDefinition>,
}

impl Default for BuildingConstructionCostCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_construction_costs())
    }
}

impl BuildingConstructionCostCatalog {
    pub fn from_definitions(defs: Vec<BuildingConstructionCostDefinition>) -> Self {
        let mut costs = HashMap::new();
        for def in defs {
            costs.insert(def.building_definition_id.clone(), def);
        }
        Self { costs }
    }

    pub fn materials_for(
        &self,
        building_definition_id: &BuildingDefinitionId,
    ) -> &[(ItemDefinitionId, u32)] {
        self.costs
            .get(building_definition_id)
            .map(|d| d.materials.as_slice())
            .unwrap_or(&[])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstructionCatalogError {
    EmptyResponseId,
    DuplicateResponseId(ResponseId),
}

impl std::fmt::Display for ConstructionCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyResponseId => write!(f, "construction mapping has empty response id"),
            Self::DuplicateResponseId(id) => {
                write!(f, "duplicate construction mapping for `{}`", id.as_str())
            }
        }
    }
}
