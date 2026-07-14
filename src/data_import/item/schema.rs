//! Excel column schema and conversion into item definitions (ADR-087 I1).

use crate::world::normalize_tags;
use crate::world::{ItemCategoryId, ItemDefinition, ItemDefinitionId, ItemIconKey, ItemRenderKey};

pub const REQUIRED_COLUMNS: &[&str] = &[
    "Item ID",
    "Name",
    "Category",
    "Width",
    "Height",
    "Stackable",
    "Max Stack",
    "Mass Grams",
    "Enabled",
];

pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Description",
    "Render Key",
    "Icon Key",
    "Base Value",
    "Tags",
    "Unique Instance Required",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ItemImportRow {
    pub row_number: usize,
    pub item_id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub width: u8,
    pub height: u8,
    pub stackable: bool,
    pub max_stack: u32,
    pub mass_grams: u32,
    pub base_value: u32,
    pub render_key: Option<String>,
    pub icon_key: Option<String>,
    pub tags: Vec<String>,
    pub unique_instance_required: bool,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl ItemImportRow {
    pub fn to_definition(&self) -> ItemDefinition {
        let mut definition = ItemDefinition::new(
            ItemDefinitionId::new(self.item_id.trim()),
            self.name.trim(),
            self.description.trim(),
            ItemCategoryId::new(self.category.trim()),
            self.width,
            self.height,
            self.stackable,
            self.max_stack,
            self.mass_grams,
            self.base_value,
            self.enabled,
        )
        .with_tags(self.tags.clone())
        .with_unique_instance_required(self.unique_instance_required);

        if let Some(key) = self
            .render_key
            .as_ref()
            .filter(|key| !key.trim().is_empty())
        {
            definition = definition.with_render_key(ItemRenderKey::reserved(key.trim()));
        }
        if let Some(key) = self.icon_key.as_ref().filter(|key| !key.trim().is_empty()) {
            definition = definition.with_icon_key(ItemIconKey::reserved(key.trim()));
        }

        definition
    }
}

pub fn parse_tags_cell(value: &str) -> Vec<String> {
    normalize_tags(value)
}
