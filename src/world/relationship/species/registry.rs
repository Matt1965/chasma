use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::SpeciesDefinition;
use super::id::SpeciesId;
use super::starter::starter_definitions;

/// Read-only registry of species identity definitions (ADR-132 Phase 1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct SpeciesCatalog {
    definitions: Vec<SpeciesDefinition>,
    by_id: HashMap<SpeciesId, usize>,
}

impl Default for SpeciesCatalog {
    fn default() -> Self {
        #[cfg(any(test, feature = "dev"))]
        {
            Self::from_definitions(starter_definitions()).expect("species catalog is valid")
        }
        #[cfg(not(any(test, feature = "dev")))]
        {
            Self::from_definitions(Vec::new()).expect("empty species catalog is valid")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeciesCatalogError {
    DuplicateId(SpeciesId),
}

impl std::fmt::Display for SpeciesCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate species id `{id}`"),
        }
    }
}

impl SpeciesCatalog {
    pub fn from_definitions(
        definitions: Vec<SpeciesDefinition>,
    ) -> Result<Self, SpeciesCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(SpeciesCatalogError::DuplicateId(definition.id.clone()));
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

    pub fn definitions(&self) -> &[SpeciesDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &SpeciesId) -> Option<&SpeciesDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn contains(&self, id: &SpeciesId) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn display_name(&self, id: &SpeciesId) -> Option<&str> {
        self.get(id)
            .map(|definition| definition.display_name.as_str())
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &SpeciesDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_species_load() {
        let catalog = SpeciesCatalog::default();
        assert!(catalog.get(&SpeciesId::new("cavecrawler")).is_some());
    }

    #[test]
    fn duplicate_species_id_rejected() {
        let defs = starter_definitions();
        let mut dup = defs.clone();
        dup.push(defs[0].clone());
        assert!(matches!(
            SpeciesCatalog::from_definitions(dup),
            Err(SpeciesCatalogError::DuplicateId(_))
        ));
    }
}
