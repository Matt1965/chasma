//! Excel column schema for faction identity (ADR-132 Phase 1).

use crate::world::{FactionDefinition, FactionId};

use super::normalize_relationship_key;

pub const FACTION_REQUIRED_COLUMNS: &[&str] = &["Faction Key", "Name", "Enabled"];

pub const FACTION_OPTIONAL_COLUMNS: &[&str] = &["Faction ID", "Description"];

/// Retired one-dimensional relationship authority — ignored when present.
pub const FACTION_RETIRED_COLUMNS: &[&str] = &["Disposition"];

#[derive(Debug, Clone, PartialEq)]
pub struct FactionImportRow {
    pub row_number: usize,
    pub faction_key: String,
    pub name: String,
    pub legacy_faction_id: String,
    pub description: String,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl FactionImportRow {
    pub fn to_definition(&self) -> Result<FactionDefinition, String> {
        let id = FactionId::new(normalize_relationship_key(&self.faction_key)?);
        let mut definition =
            FactionDefinition::new(id, self.name.trim(), self.description.trim(), self.enabled);
        let legacy = self.legacy_faction_id.trim();
        if !legacy.is_empty() {
            definition = definition.with_legacy_faction_id(legacy);
        }
        Ok(definition)
    }
}
