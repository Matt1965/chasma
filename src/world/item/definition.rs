use bevy::prelude::*;

use super::category_id::ItemCategoryId;
use super::definition_id::ItemDefinitionId;
use super::icon_key::ItemIconKey;
use super::render_key::ItemRenderKey;
use crate::world::InventoryProfileId;
use crate::world::WeaponDefinitionId;

/// Authoritative description of a physical item type (ADR-087 I1).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ItemDefinition {
    pub id: ItemDefinitionId,
    pub display_name: String,
    pub description: String,
    pub category_id: ItemCategoryId,
    pub grid_width: u8,
    pub grid_height: u8,
    pub stackable: bool,
    pub max_stack: u32,
    /// Integer mass per unit in grams.
    pub mass_grams_per_unit: u32,
    pub render_key: ItemRenderKey,
    pub icon_key: ItemIconKey,
    pub base_value_gold: u32,
    pub tags: Vec<String>,
    pub unique_instance_required: bool,
    pub enabled: bool,
    /// Future combat/equipment references — unset in I1.
    pub weapon_definition_id: Option<WeaponDefinitionId>,
    pub armor_profile_id: Option<String>,
    pub consumable_profile_id: Option<String>,
    pub backpack_profile_id: Option<InventoryProfileId>,
    pub container_profile_id: Option<InventoryProfileId>,
    pub quality_profile_id: Option<String>,
}

impl ItemDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ItemDefinitionId,
        display_name: impl Into<String>,
        description: impl Into<String>,
        category_id: ItemCategoryId,
        grid_width: u8,
        grid_height: u8,
        stackable: bool,
        max_stack: u32,
        mass_grams_per_unit: u32,
        base_value_gold: u32,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            description: description.into(),
            category_id,
            grid_width,
            grid_height,
            stackable,
            max_stack,
            mass_grams_per_unit,
            render_key: ItemRenderKey::unset(),
            icon_key: ItemIconKey::unset(),
            base_value_gold,
            tags: Vec::new(),
            unique_instance_required: false,
            enabled,
            weapon_definition_id: None,
            armor_profile_id: None,
            consumable_profile_id: None,
            backpack_profile_id: None,
            container_profile_id: None,
            quality_profile_id: None,
        }
    }

    pub fn with_render_key(mut self, render_key: ItemRenderKey) -> Self {
        self.render_key = render_key;
        self
    }

    pub fn with_icon_key(mut self, icon_key: ItemIconKey) -> Self {
        self.icon_key = icon_key;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_unique_instance_required(mut self, required: bool) -> Self {
        self.unique_instance_required = required;
        self
    }
}
