//! Excel column schema and conversion into item definitions (ADR-087 I1).

use crate::world::equipment::EquipmentSlot;
use crate::world::normalize_tags;
use crate::world::{
    ArmorProfileId, InventoryProfileId, ItemCategoryId, ItemDefinition, ItemDefinitionId,
    ItemIconKey, ItemRenderKey, WeaponDefinitionId,
};

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
    "Nutrition",
    "Equipment Slots",
    "Weapon Definition ID",
    "Armor Profile ID",
    "Backpack Profile ID",
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
    pub nutrition: u32,
    pub equipment_slots: Vec<EquipmentSlot>,
    pub weapon_definition_id: Option<String>,
    pub armor_profile_id: Option<String>,
    pub backpack_profile_id: Option<String>,
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

        if let Some(weapon_id) = self
            .weapon_definition_id
            .as_ref()
            .filter(|id| !id.trim().is_empty())
        {
            definition = definition.with_weapon_definition_id(WeaponDefinitionId::new(weapon_id));
        }
        if let Some(armor_id) = self
            .armor_profile_id
            .as_ref()
            .filter(|id| !id.trim().is_empty())
        {
            definition = definition.with_armor_profile_id(ArmorProfileId::new(armor_id));
        }
        if let Some(profile_id) = self
            .backpack_profile_id
            .as_ref()
            .filter(|id| !id.trim().is_empty())
        {
            definition = definition.with_backpack_profile_id(InventoryProfileId::new(profile_id));
        }

        definition
            .with_nutrition(self.nutrition)
            .with_equipment_slots(self.equipment_slots.clone())
    }
}

pub fn parse_tags_cell(value: &str) -> Vec<String> {
    normalize_tags(value)
}
