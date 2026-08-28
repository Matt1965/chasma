use bevy::prelude::*;

use super::id::FactionId;

/// Authoritative faction identity metadata (ADR-132 Phase 1).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct FactionDefinition {
    pub id: FactionId,
    pub display_name: String,
    /// Legacy workbook cross-reference (`F-0001`); not a relationship identity.
    pub legacy_faction_id: Option<String>,
    pub description: String,
    pub enabled: bool,
}

impl FactionDefinition {
    pub fn new(
        id: FactionId,
        display_name: impl Into<String>,
        description: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            description: description.into(),
            legacy_faction_id: None,
            enabled,
        }
    }

    pub fn with_legacy_faction_id(mut self, legacy_id: impl Into<String>) -> Self {
        self.legacy_faction_id = Some(legacy_id.into());
        self
    }
}
