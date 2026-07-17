//! Read-only registry of field response profiles (ADR-104 TF4).

use std::collections::BTreeMap;
use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::definition::FieldResponseProfileDefinition;
use super::error::FieldResponseProfileError;
use super::id::FieldResponseProfileId;
use super::starter;

/// Monotonic revision bumped when catalog content changes (hot reload seam).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect, Resource)]
pub struct FieldResponseProfileCatalogRevision(pub u64);

/// Read-only response profile catalog.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct FieldResponseProfileCatalog {
    definitions: Vec<FieldResponseProfileDefinition>,
    #[reflect(ignore)]
    by_id: BTreeMap<FieldResponseProfileId, usize>,
}

impl Default for FieldResponseProfileCatalog {
    fn default() -> Self {
        Self::from_definitions(starter::starter_profiles())
            .expect("starter field response profiles are valid")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldResponseProfileCatalogRon {
    pub definitions: Vec<FieldResponseProfileDefinition>,
}

impl FieldResponseProfileCatalog {
    pub fn from_definitions(
        definitions: Vec<FieldResponseProfileDefinition>,
    ) -> Result<Self, FieldResponseProfileError> {
        let mut by_id = BTreeMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            definition.validate()?;
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(FieldResponseProfileError::DuplicateId(
                    definition.id.clone(),
                ));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn get(&self, id: &FieldResponseProfileId) -> Option<&FieldResponseProfileDefinition> {
        self.by_id.get(id).map(|index| &self.definitions[*index])
    }

    pub fn definitions(&self) -> &[FieldResponseProfileDefinition] {
        &self.definitions
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &FieldResponseProfileDefinition> {
        self.definitions.iter().filter(|def| def.enabled)
    }

    pub fn load_from_ron_path(path: &Path) -> Result<Self, FieldResponseProfileError> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| FieldResponseProfileError::RonIo(err.to_string()))?;
        Self::load_from_ron(&text)
    }

    pub fn load_from_ron(text: &str) -> Result<Self, FieldResponseProfileError> {
        let file: FieldResponseProfileCatalogRon = ron::from_str(text)
            .map_err(|err| FieldResponseProfileError::RonParse(err.to_string()))?;
        Self::from_definitions(file.definitions)
    }
}

pub const FIELD_RESPONSE_PROFILE_CATALOG_RON_PATH: &str =
    "assets/field_response_profiles/catalog.ron";

pub fn load_field_response_profile_catalog() -> FieldResponseProfileCatalog {
    FieldResponseProfileCatalog::load_from_ron_path(Path::new(
        FIELD_RESPONSE_PROFILE_CATALOG_RON_PATH,
    ))
    .unwrap_or_else(|err| {
        bevy::log::warn!(
            "field response profile catalog missing or invalid at {FIELD_RESPONSE_PROFILE_CATALOG_RON_PATH} ({err}); using starter profiles"
        );
        FieldResponseProfileCatalog::default()
    })
}
