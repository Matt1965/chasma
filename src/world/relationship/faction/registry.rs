use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::FactionDefinition;
use super::id::FactionId;
use super::starter::starter_definitions;

/// Read-only registry of faction identity definitions (ADR-132 Phase 1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct FactionCatalog {
    definitions: Vec<FactionDefinition>,
    by_id: HashMap<FactionId, usize>,
}

impl Default for FactionCatalog {
    fn default() -> Self {
        #[cfg(any(test, feature = "dev"))]
        {
            Self::from_definitions(starter_definitions()).expect("faction catalog is valid")
        }
        #[cfg(not(any(test, feature = "dev")))]
        {
            Self::from_definitions(Vec::new()).expect("empty faction catalog is valid")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactionCatalogError {
    DuplicateId(FactionId),
}

impl std::fmt::Display for FactionCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate faction id `{id}`"),
        }
    }
}

impl FactionCatalog {
    pub fn from_definitions(
        definitions: Vec<FactionDefinition>,
    ) -> Result<Self, FactionCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(FactionCatalogError::DuplicateId(definition.id.clone()));
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

    pub fn definitions(&self) -> &[FactionDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &FactionId) -> Option<&FactionDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn contains(&self, id: &FactionId) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn display_name(&self, id: &FactionId) -> Option<&str> {
        self.get(id)
            .map(|definition| definition.display_name.as_str())
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &FactionDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_factions_load() {
        let catalog = FactionCatalog::default();
        assert!(catalog.get(&FactionId::new("wild")).is_some());
    }

    #[test]
    fn duplicate_faction_id_rejected() {
        let defs = starter_definitions();
        let mut dup = defs.clone();
        dup.push(defs[0].clone());
        assert!(matches!(
            FactionCatalog::from_definitions(dup),
            Err(FactionCatalogError::DuplicateId(_))
        ));
    }
}
