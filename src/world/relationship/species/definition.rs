use bevy::prelude::*;

use super::id::SpeciesId;

/// Authoritative species identity metadata (ADR-132 Phase 1).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SpeciesDefinition {
    pub id: SpeciesId,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
}

impl SpeciesDefinition {
    pub fn new(
        id: SpeciesId,
        display_name: impl Into<String>,
        description: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            description: description.into(),
            enabled,
        }
    }
}
