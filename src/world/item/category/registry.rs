use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::ItemCategoryDefinition;
use super::starter::starter_definitions;
use crate::world::ItemCategoryId;

/// Read-only registry of item category definitions (ADR-087 I1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct ItemCategoryCatalog {
    definitions: Vec<ItemCategoryDefinition>,
    by_id: HashMap<ItemCategoryId, usize>,
}

impl Default for ItemCategoryCatalog {
    fn default() -> Self {
        #[cfg(any(test, feature = "dev"))]
        {
            Self::from_definitions(starter_definitions()).expect("item category catalog is valid")
        }
        #[cfg(not(any(test, feature = "dev")))]
        {
            Self::from_definitions(Vec::new()).expect("empty item category catalog is valid")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemCategoryCatalogError {
    DuplicateId(ItemCategoryId),
}

impl std::fmt::Display for ItemCategoryCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => {
                write!(f, "duplicate item category id `{id}`", id = id.as_str())
            }
        }
    }
}

impl ItemCategoryCatalog {
    pub fn from_definitions(
        definitions: Vec<ItemCategoryDefinition>,
    ) -> Result<Self, ItemCategoryCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(ItemCategoryCatalogError::DuplicateId(definition.id.clone()));
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

    pub fn definitions(&self) -> &[ItemCategoryDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &ItemCategoryId) -> Option<&ItemCategoryDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn contains(&self, id: &ItemCategoryId) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &ItemCategoryDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_categories_load() {
        let catalog = ItemCategoryCatalog::default();
        assert!(catalog.get(&ItemCategoryId::new("currency")).is_some());
    }

    #[test]
    fn duplicate_category_id_rejected() {
        let defs = starter_definitions();
        let mut dup = defs.clone();
        dup.push(defs[0].clone());
        assert!(matches!(
            ItemCategoryCatalog::from_definitions(dup),
            Err(ItemCategoryCatalogError::DuplicateId(_))
        ));
    }
}
