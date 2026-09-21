use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::AppearanceProfile;
use super::id::AppearanceProfileId;

/// Read-only registry of appearance profiles.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct AppearanceProfileCatalog {
    definitions: Vec<AppearanceProfile>,
    by_id: HashMap<AppearanceProfileId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppearanceProfileCatalogError {
    DuplicateId(AppearanceProfileId),
}

impl AppearanceProfileCatalog {
    pub fn from_definitions(
        definitions: Vec<AppearanceProfile>,
    ) -> Result<Self, AppearanceProfileCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(AppearanceProfileCatalogError::DuplicateId(
                    definition.id.clone(),
                ));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn empty() -> Self {
        Self::from_definitions(Vec::new()).expect("empty appearance catalog is valid")
    }

    pub fn get(&self, id: &AppearanceProfileId) -> Option<&AppearanceProfile> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn definitions(&self) -> &[AppearanceProfile] {
        &self.definitions
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

impl Default for AppearanceProfileCatalog {
    fn default() -> Self {
        Self::empty()
    }
}
