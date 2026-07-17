//! Terrain field catalog registry (ADR-101).

use std::collections::BTreeMap;
use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::definition::TerrainFieldDefinition;
use super::super::error::{TerrainFieldCatalogError, TerrainFieldDefinitionError};
use super::super::id::TerrainFieldId;
use super::starter;

/// Read-only registry of terrain field definitions.
#[derive(Debug, Clone, Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct TerrainFieldCatalog {
    definitions: Vec<TerrainFieldDefinition>,
    #[reflect(ignore)]
    by_id: BTreeMap<TerrainFieldId, usize>,
}

impl Default for TerrainFieldCatalog {
    fn default() -> Self {
        Self::from_definitions(starter::starter_definitions())
            .expect("starter terrain field catalog is valid")
    }
}

impl TerrainFieldCatalog {
    pub fn from_definitions(
        definitions: Vec<TerrainFieldDefinition>,
    ) -> Result<Self, TerrainFieldCatalogError> {
        let mut by_id = BTreeMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            definition
                .overlay_style
                .validate()
                .map_err(TerrainFieldCatalogError::InvalidDefinition)?;
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(TerrainFieldCatalogError::DuplicateId(definition.id.clone()));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn get(&self, id: &TerrainFieldId) -> Option<&TerrainFieldDefinition> {
        self.by_id.get(id).map(|index| &self.definitions[*index])
    }

    pub fn definitions(&self) -> &[TerrainFieldDefinition] {
        &self.definitions
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &TerrainFieldDefinition> {
        self.definitions.iter().filter(|def| def.enabled)
    }

    pub fn sorted_ids(&self) -> Vec<TerrainFieldId> {
        self.by_id.keys().cloned().collect()
    }

    pub fn load_from_ron_path(path: &Path) -> Result<Self, TerrainFieldCatalogError> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| TerrainFieldCatalogError::RonIo(err.to_string()))?;
        Self::load_from_ron(&text)
    }

    pub fn load_from_ron(text: &str) -> Result<Self, TerrainFieldCatalogError> {
        let file: TerrainFieldCatalogRon = ron::from_str(text)
            .map_err(|err| TerrainFieldCatalogError::RonParse(err.to_string()))?;
        Self::from_definitions(file.definitions)
    }
}

/// Serializable catalog DTO for committed RON assets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldCatalogRon {
    pub definitions: Vec<TerrainFieldDefinition>,
}

pub fn validate_terrain_field_id(id: &str) -> Result<(), TerrainFieldDefinitionError> {
    let trimmed = id.trim();
    if trimmed.is_empty() || trimmed != id.trim().to_lowercase() {
        return Err(TerrainFieldDefinitionError::InvalidTerrainFieldId(
            id.to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(TerrainFieldDefinitionError::InvalidTerrainFieldId(
            id.to_string(),
        ));
    }
    Ok(())
}
