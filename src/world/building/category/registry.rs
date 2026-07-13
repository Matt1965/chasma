use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::BuildingCategoryDefinition;
use super::definition_id::BuildingCategoryId;
use super::starter::starter_definitions;

/// Read-only registry of building category definitions (B1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingCategoryCatalog {
    definitions: Vec<BuildingCategoryDefinition>,
    by_id: HashMap<BuildingCategoryId, usize>,
}

impl Default for BuildingCategoryCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_definitions()).expect("building category catalog is valid")
    }
}

/// Why [`BuildingCategoryCatalog::from_definitions`] rejected input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingCategoryCatalogError {
    DuplicateId(BuildingCategoryId),
}

impl std::fmt::Display for BuildingCategoryCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate building category id `{id}`"),
        }
    }
}

impl BuildingCategoryCatalog {
    pub fn from_definitions(
        definitions: Vec<BuildingCategoryDefinition>,
    ) -> Result<Self, BuildingCategoryCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(BuildingCategoryCatalogError::DuplicateId(
                    definition.id.clone(),
                ));
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

    pub fn definitions(&self) -> &[BuildingCategoryDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &BuildingCategoryId) -> Option<&BuildingCategoryDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn contains(&self, id: &BuildingCategoryId) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &BuildingCategoryDefinition> {
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
        let catalog = BuildingCategoryCatalog::default();
        assert!(
            catalog
                .get(&BuildingCategoryId::new("residential"))
                .is_some()
        );
    }

    #[test]
    fn duplicate_category_id_rejected() {
        let defs = starter_definitions();
        let mut dup = defs.clone();
        dup.push(defs[0].clone());
        assert!(matches!(
            BuildingCategoryCatalog::from_definitions(dup),
            Err(BuildingCategoryCatalogError::DuplicateId(_))
        ));
    }
}
