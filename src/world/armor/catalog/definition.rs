use bevy::prelude::*;

use super::definition_id::ArmorProfileId;

/// Authoritative armor profile definition.
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct ArmorProfileDefinition {
    pub id: ArmorProfileId,
    pub display_name: String,
    pub description: String,
    pub armor_rating: u32,
    pub enabled: bool,
}

impl ArmorProfileDefinition {
    pub fn new(
        id: ArmorProfileId,
        display_name: impl Into<String>,
        description: impl Into<String>,
        armor_rating: u32,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            description: description.into(),
            armor_rating,
            enabled,
        }
    }
}
