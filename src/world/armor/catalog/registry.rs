use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::ArmorProfileDefinition;
use super::definition_id::ArmorProfileId;
/// Read-only registry of armor profile definitions (Slice 3 seam).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct ArmorProfileCatalog {
    definitions: Vec<ArmorProfileDefinition>,
    by_id: HashMap<ArmorProfileId, usize>,
}

impl Default for ArmorProfileCatalog {
    fn default() -> Self {
        Self::from_definitions(Vec::new()).expect("empty armor profile catalog is valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArmorProfileCatalogError {
    DuplicateId(ArmorProfileId),
}

impl std::fmt::Display for ArmorProfileCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => {
                write!(f, "duplicate armor profile id `{id}`", id = id.as_str())
            }
        }
    }
}

impl ArmorProfileCatalog {
    pub fn from_definitions(
        definitions: Vec<ArmorProfileDefinition>,
    ) -> Result<Self, ArmorProfileCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(ArmorProfileCatalogError::DuplicateId(definition.id.clone()));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn definitions(&self) -> &[ArmorProfileDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &ArmorProfileId) -> Option<&ArmorProfileDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }
}
