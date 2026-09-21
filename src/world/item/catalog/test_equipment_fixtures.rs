//! Isolated equipment item fixtures for unit tests only.

use crate::world::armor::ArmorProfileId;
use crate::world::equipment::EquipmentSlot;
use crate::world::{
    InventoryProfileId, ItemCategoryId, ItemDefinition, ItemDefinitionId, WeaponDefinitionId,
};

pub fn test_equipment_fixture_definitions() -> Vec<ItemDefinition> {
    vec![
        unique_item(
            "iron_sword",
            "Iron Sword",
            "weapon",
            1,
            3,
            1_500,
            vec![EquipmentSlot::Weapon],
            Some(WeaponDefinitionId::new("weapon_iron_sword")),
            None,
            None,
        ),
        unique_item(
            "leather_backpack",
            "Leather Backpack",
            "container",
            2,
            3,
            500,
            vec![EquipmentSlot::Backpack],
            None,
            None,
            Some(InventoryProfileId::new("backpack_basic_internal")),
        ),
        unique_item(
            "ranger_hood",
            "Ranger Hood",
            "armor",
            2,
            2,
            800,
            vec![EquipmentSlot::Head],
            None,
            Some(ArmorProfileId::new("armor_ranger_hood")),
            None,
        ),
        unique_item(
            "ranger_body",
            "Ranger Body",
            "armor",
            3,
            3,
            1_200,
            vec![EquipmentSlot::Body],
            None,
            Some(ArmorProfileId::new("armor_ranger_body")),
            None,
        ),
        unique_item(
            "ranger_arms",
            "Ranger Arms",
            "armor",
            2,
            2,
            600,
            vec![EquipmentSlot::Arms],
            None,
            Some(ArmorProfileId::new("armor_ranger_arms")),
            None,
        ),
        unique_item(
            "ranger_legs",
            "Ranger Legs",
            "armor",
            2,
            3,
            900,
            vec![EquipmentSlot::Legs],
            None,
            Some(ArmorProfileId::new("armor_ranger_legs")),
            None,
        ),
        unique_item(
            "ranger_feet",
            "Ranger Feet",
            "armor",
            2,
            2,
            700,
            vec![EquipmentSlot::Feet],
            None,
            Some(ArmorProfileId::new("armor_ranger_feet")),
            None,
        ),
    ]
}

fn unique_item(
    id: &str,
    name: &str,
    category: &str,
    width: u8,
    height: u8,
    mass: u32,
    slots: Vec<EquipmentSlot>,
    weapon_id: Option<WeaponDefinitionId>,
    armor_id: Option<ArmorProfileId>,
    backpack_profile: Option<InventoryProfileId>,
) -> ItemDefinition {
    let mut definition = ItemDefinition::new(
        ItemDefinitionId::new(id),
        name,
        "",
        ItemCategoryId::new(category),
        width,
        height,
        false,
        1,
        mass,
        5,
        true,
    )
    .with_unique_instance_required(true)
    .with_equipment_slots(slots);
    if let Some(weapon_id) = weapon_id {
        definition = definition.with_weapon_definition_id(weapon_id);
    }
    if let Some(armor_id) = armor_id {
        definition = definition.with_armor_profile_id(armor_id);
    }
    if let Some(profile) = backpack_profile {
        definition = definition.with_backpack_profile_id(profile);
    }
    definition
}
