use std::collections::HashMap;

use bevy::prelude::*;

use super::starter::starter_definitions;
use crate::world::ItemCategoryCatalog;
use crate::world::ItemDefinition;
use crate::world::ItemDefinitionId;
use crate::world::{ItemValidationError, validate_item_definition};

/// Read-only registry of item type definitions (ADR-087 I1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct ItemCatalog {
    definitions: Vec<ItemDefinition>,
    by_id: HashMap<ItemDefinitionId, usize>,
}

impl Default for ItemCatalog {
    fn default() -> Self {
        #[cfg(any(test, feature = "dev"))]
        {
            Self::from_definitions_with_categories(
                starter_definitions(),
                &ItemCategoryCatalog::default(),
            )
            .expect("item catalog is valid")
        }
        #[cfg(not(any(test, feature = "dev")))]
        {
            Self::from_definitions(Vec::new(), &ItemCategoryCatalog::default())
                .expect("empty item catalog is valid")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemCatalogError {
    DuplicateId(ItemDefinitionId),
    Validation(ItemValidationError),
}

impl std::fmt::Display for ItemCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate item id `{id}`", id = id.as_str()),
            Self::Validation(err) => write!(f, "{err}"),
        }
    }
}

impl ItemCatalog {
    pub fn from_definitions(
        definitions: Vec<ItemDefinition>,
        categories: &ItemCategoryCatalog,
    ) -> Result<Self, ItemCatalogError> {
        Self::from_definitions_with_categories(definitions, categories)
    }

    pub fn from_definitions_with_categories(
        definitions: Vec<ItemDefinition>,
        categories: &ItemCategoryCatalog,
    ) -> Result<Self, ItemCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());

        for (index, definition) in definitions.iter().enumerate() {
            validate_item_definition(definition, categories, None)
                .map_err(ItemCatalogError::Validation)?;
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(ItemCatalogError::DuplicateId(definition.id.clone()));
            }
        }

        Ok(Self { definitions, by_id })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn definitions(&self) -> &[ItemDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &ItemDefinitionId) -> Option<&ItemDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &ItemDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ItemCategoryId;

    #[test]
    fn catalog_contains_physical_gold() {
        let catalog = ItemCatalog::default();
        let gold = catalog
            .get(&ItemDefinitionId::new("gold"))
            .expect("physical gold");
        assert!(gold.stackable);
        assert_eq!(gold.category_id, ItemCategoryId::new("currency"));
    }

    #[test]
    fn duplicate_item_id_rejected() {
        let defs = starter_definitions();
        let mut dup = defs.clone();
        dup.push(defs[0].clone());
        assert!(matches!(
            ItemCatalog::from_definitions_with_categories(dup, &ItemCategoryCatalog::default()),
            Err(ItemCatalogError::DuplicateId(_))
        ));
    }

    #[test]
    fn deterministic_iteration_matches_insertion_order() {
        let catalog = ItemCatalog::default();
        let ids: Vec<_> = catalog
            .definitions()
            .iter()
            .map(|def| def.id.as_str().to_string())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_ne!(ids, sorted);
    }

    #[test]
    fn lookup_by_stable_id() {
        let catalog = ItemCatalog::default();
        assert!(catalog.get(&ItemDefinitionId::new("gold")).is_some());
    }
}
