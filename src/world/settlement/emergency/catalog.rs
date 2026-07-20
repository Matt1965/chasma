//! EmergencyDefinition catalog (SA8).

use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::{EmergencyDefinition, EmergencyId};
use super::starter::starter_emergency_definitions;

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct EmergencyCatalog {
    definitions: Vec<EmergencyDefinition>,
    by_id: HashMap<EmergencyId, usize>,
}

impl Default for EmergencyCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_emergency_definitions())
            .expect("starter emergency definitions are valid")
    }
}

impl EmergencyCatalog {
    pub fn from_definitions(
        definitions: Vec<EmergencyDefinition>,
    ) -> Result<Self, EmergencyCatalogError> {
        let mut by_id = HashMap::new();
        for (index, def) in definitions.iter().enumerate() {
            if def.id.as_str().is_empty() {
                return Err(EmergencyCatalogError::EmptyEmergencyId);
            }
            if by_id.insert(def.id.clone(), index).is_some() {
                return Err(EmergencyCatalogError::DuplicateEmergencyId(def.id.clone()));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn definitions(&self) -> &[EmergencyDefinition] {
        &self.definitions
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &EmergencyDefinition> {
        self.definitions.iter().filter(|d| d.enabled)
    }

    pub fn get(&self, id: &EmergencyId) -> Option<&EmergencyDefinition> {
        self.by_id.get(id).map(|&i| &self.definitions[i])
    }

    pub fn get_str(&self, id: &str) -> Option<&EmergencyDefinition> {
        self.get(&EmergencyId::new(id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmergencyCatalogError {
    EmptyEmergencyId,
    DuplicateEmergencyId(EmergencyId),
}

impl std::fmt::Display for EmergencyCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyEmergencyId => write!(f, "emergency definition has empty id"),
            Self::DuplicateEmergencyId(id) => {
                write!(f, "duplicate EmergencyId `{}`", id.as_str())
            }
        }
    }
}
