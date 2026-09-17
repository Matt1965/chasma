use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::slot::EquipmentSlot;
use crate::world::InventoryId;

/// Authoritative equipment-slot inventory links for one unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct UnitEquipmentInventories {
    pub head: InventoryId,
    pub body: InventoryId,
    pub arms: InventoryId,
    pub legs: InventoryId,
    pub feet: InventoryId,
    pub weapon: InventoryId,
    pub offhand: InventoryId,
    pub backpack: InventoryId,
}

impl UnitEquipmentInventories {
    pub fn inventory_id(&self, slot: EquipmentSlot) -> InventoryId {
        match slot {
            EquipmentSlot::Head => self.head,
            EquipmentSlot::Body => self.body,
            EquipmentSlot::Arms => self.arms,
            EquipmentSlot::Legs => self.legs,
            EquipmentSlot::Feet => self.feet,
            EquipmentSlot::Weapon => self.weapon,
            EquipmentSlot::Offhand => self.offhand,
            EquipmentSlot::Backpack => self.backpack,
        }
    }

    pub fn set_inventory_id(&mut self, slot: EquipmentSlot, inventory_id: InventoryId) {
        match slot {
            EquipmentSlot::Head => self.head = inventory_id,
            EquipmentSlot::Body => self.body = inventory_id,
            EquipmentSlot::Arms => self.arms = inventory_id,
            EquipmentSlot::Legs => self.legs = inventory_id,
            EquipmentSlot::Feet => self.feet = inventory_id,
            EquipmentSlot::Weapon => self.weapon = inventory_id,
            EquipmentSlot::Offhand => self.offhand = inventory_id,
            EquipmentSlot::Backpack => self.backpack = inventory_id,
        }
    }

    pub fn all_inventory_ids(&self) -> [InventoryId; 8] {
        [
            self.head,
            self.body,
            self.arms,
            self.legs,
            self.feet,
            self.weapon,
            self.offhand,
            self.backpack,
        ]
    }

    pub fn contains_inventory(&self, inventory_id: InventoryId) -> Option<EquipmentSlot> {
        for slot in EquipmentSlot::ALL {
            if self.inventory_id(slot) == inventory_id {
                return Some(slot);
            }
        }
        None
    }
}
