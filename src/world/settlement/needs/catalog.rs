//! Read-only NeedDefinition registry (SA2).

use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::{NeedDefinition, NeedEvaluationMethod};
use super::id::NeedId;
use super::starter::starter_need_definitions;
use super::validation::NeedCatalogError;

/// Immutable catalog of authored need definitions.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct NeedCatalog {
    definitions: Vec<NeedDefinition>,
    by_id: HashMap<NeedId, usize>,
}

impl Default for NeedCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_need_definitions()).expect("need catalog is valid")
    }
}

impl NeedCatalog {
    pub fn from_definitions(definitions: Vec<NeedDefinition>) -> Result<Self, NeedCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if definition.id.as_str().is_empty() {
                return Err(NeedCatalogError::EmptyNeedId);
            }
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(NeedCatalogError::DuplicateNeedId(definition.id.clone()));
            }
            if !evaluation_method_known(definition.evaluation_method) {
                return Err(NeedCatalogError::UnknownEvaluator(
                    definition.evaluation_method,
                ));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn definitions(&self) -> &[NeedDefinition] {
        &self.definitions
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &NeedDefinition> {
        self.definitions.iter().filter(|d| d.enabled)
    }

    pub fn get(&self, id: &NeedId) -> Option<&NeedDefinition> {
        self.by_id.get(id).map(|&i| &self.definitions[i])
    }

    pub fn get_str(&self, id: &str) -> Option<&NeedDefinition> {
        self.get(&NeedId::new(id))
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

fn evaluation_method_known(method: NeedEvaluationMethod) -> bool {
    matches!(
        method,
        NeedEvaluationMethod::FoodStock
            | NeedEvaluationMethod::ConstructionSites
            | NeedEvaluationMethod::HousingCapacity
            | NeedEvaluationMethod::DefensePosture
            | NeedEvaluationMethod::ResearchStub
            | NeedEvaluationMethod::ExpansionGrowth
            | NeedEvaluationMethod::LuxuryStock
    )
}
