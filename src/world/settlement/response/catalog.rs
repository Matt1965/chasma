//! Read-only ResponseDefinition registry (SA3).

use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::ResponseDefinition;
use super::id::ResponseId;
use super::starter::starter_response_definitions;
use super::validation::{validate_response_catalog_definitions, ResponseCatalogError};
use crate::world::settlement::needs::NeedId;

/// Immutable catalog of authored response definitions.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct ResponseCatalog {
    definitions: Vec<ResponseDefinition>,
    by_id: HashMap<ResponseId, usize>,
    by_need: HashMap<NeedId, Vec<usize>>,
}

impl Default for ResponseCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_response_definitions()).expect("response catalog is valid")
    }
}

impl ResponseCatalog {
    pub fn from_definitions(
        definitions: Vec<ResponseDefinition>,
    ) -> Result<Self, ResponseCatalogError> {
        validate_response_catalog_definitions(&definitions)?;
        let mut by_id = HashMap::with_capacity(definitions.len());
        let mut by_need: HashMap<NeedId, Vec<usize>> = HashMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            by_id.insert(definition.id.clone(), index);
            for need_id in &definition.supported_need_ids {
                by_need.entry(need_id.clone()).or_default().push(index);
            }
        }
        Ok(Self {
            definitions,
            by_id,
            by_need,
        })
    }

    pub fn definitions(&self) -> &[ResponseDefinition] {
        &self.definitions
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &ResponseDefinition> {
        self.definitions.iter().filter(|d| d.enabled)
    }

    pub fn get(&self, id: &ResponseId) -> Option<&ResponseDefinition> {
        self.by_id.get(id).map(|&i| &self.definitions[i])
    }

    pub fn get_str(&self, id: &str) -> Option<&ResponseDefinition> {
        self.get(&ResponseId::new(id))
    }

    /// Responses that list `need_id` in `supported_need_ids` (catalog-driven discovery).
    pub fn definitions_for_need(&self, need_id: &NeedId) -> Vec<&ResponseDefinition> {
        self.by_need
            .get(need_id)
            .into_iter()
            .flatten()
            .filter_map(|&i| {
                let def = &self.definitions[i];
                def.enabled.then_some(def)
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}
