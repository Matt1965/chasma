//! Excel column schema for species identity (ADR-132 Phase 1).

use crate::world::{SpeciesDefinition, SpeciesId};

use super::normalize_relationship_key;

pub const SPECIES_REQUIRED_COLUMNS: &[&str] = &["Species Key", "Name", "Enabled"];

pub const SPECIES_OPTIONAL_COLUMNS: &[&str] = &["Description"];

#[derive(Debug, Clone, PartialEq)]
pub struct SpeciesImportRow {
    pub row_number: usize,
    pub species_key: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl SpeciesImportRow {
    pub fn to_definition(&self) -> Result<SpeciesDefinition, String> {
        let id = SpeciesId::new(normalize_relationship_key(&self.species_key)?);
        Ok(SpeciesDefinition::new(
            id,
            self.name.trim(),
            self.description.trim(),
            self.enabled,
        ))
    }
}
