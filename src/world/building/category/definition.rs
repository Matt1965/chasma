use bevy::prelude::*;

use super::definition_id::BuildingCategoryId;

/// Authoritative grouping metadata for building definitions (B1).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingCategoryDefinition {
    pub id: BuildingCategoryId,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
}

impl BuildingCategoryDefinition {
    pub fn new(
        id: BuildingCategoryId,
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
