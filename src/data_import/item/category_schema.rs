//! Excel column schema for item categories (ADR-087 I1).

use crate::world::{ItemCategoryDefinition, ItemCategoryId};

pub const REQUIRED_COLUMNS: &[&str] = &["Category ID", "Name", "Enabled"];

pub const OPTIONAL_COLUMNS: &[&str] = &["Description", "Sort Priority"];

#[derive(Debug, Clone, PartialEq)]
pub struct ItemCategoryImportRow {
    pub row_number: usize,
    pub category_id: String,
    pub name: String,
    pub description: String,
    pub sort_priority: Option<u32>,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl ItemCategoryImportRow {
    pub fn to_definition(&self) -> ItemCategoryDefinition {
        let mut definition = ItemCategoryDefinition::new(
            ItemCategoryId::new(self.category_id.trim()),
            self.name.trim(),
            self.description.trim(),
            self.enabled,
        );
        if let Some(priority) = self.sort_priority {
            definition = definition.with_sort_priority(priority);
        }
        definition
    }
}
