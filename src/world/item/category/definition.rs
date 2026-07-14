use bevy::prelude::*;

use super::super::category_id::ItemCategoryId;

/// Authoritative grouping metadata for item definitions (ADR-087 I1).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ItemCategoryDefinition {
    pub id: ItemCategoryId,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
    pub sort_priority: Option<u32>,
}

impl ItemCategoryDefinition {
    pub fn new(
        id: ItemCategoryId,
        display_name: impl Into<String>,
        description: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            description: description.into(),
            enabled,
            sort_priority: None,
        }
    }

    pub fn with_sort_priority(mut self, priority: u32) -> Self {
        self.sort_priority = Some(priority);
        self
    }
}
