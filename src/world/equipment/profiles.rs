use crate::world::InventoryProfileId;
use crate::world::inventory::{InventoryAccessType, InventoryProfileDefinition};

use super::slot::EquipmentSlot;

/// Data-authored equipment-slot inventory profiles (grid dimensions + constraints).
pub fn equipment_slot_profile_definitions() -> Vec<InventoryProfileDefinition> {
    vec![
        equipment_slot_profile(EquipmentSlot::Head, 2, 2),
        equipment_slot_profile(EquipmentSlot::Body, 3, 3),
        equipment_slot_profile(EquipmentSlot::Arms, 2, 2),
        equipment_slot_profile(EquipmentSlot::Legs, 2, 3),
        equipment_slot_profile(EquipmentSlot::Feet, 2, 2),
        equipment_slot_profile(EquipmentSlot::Weapon, 2, 4),
        equipment_slot_profile(EquipmentSlot::Offhand, 2, 3),
        equipment_slot_profile(EquipmentSlot::Backpack, 2, 3),
        InventoryProfileDefinition::new(
            InventoryProfileId::new("backpack_basic_internal"),
            "Basic Backpack Internal",
            4,
            4,
            true,
        )
        .with_access_type(InventoryAccessType::OwnerOnly),
    ]
}

fn equipment_slot_profile(
    slot: EquipmentSlot,
    width: u8,
    height: u8,
) -> InventoryProfileDefinition {
    InventoryProfileDefinition::new(slot.profile_id(), slot.display_name(), width, height, true)
        .with_equipment_slot(slot)
        .with_max_placed_entries(1)
        .with_access_type(InventoryAccessType::OwnerOnly)
}
