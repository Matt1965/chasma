use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::InventoryProfileId;

/// Canonical semantic equipment slot (fixed vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum EquipmentSlot {
    Head,
    Body,
    Arms,
    Legs,
    Feet,
    Weapon,
    Offhand,
    Backpack,
}

impl EquipmentSlot {
    pub const ALL: [Self; 8] = [
        Self::Head,
        Self::Body,
        Self::Arms,
        Self::Legs,
        Self::Feet,
        Self::Weapon,
        Self::Offhand,
        Self::Backpack,
    ];

    pub fn profile_id(self) -> InventoryProfileId {
        InventoryProfileId::new(self.profile_id_str())
    }

    pub fn profile_id_str(self) -> &'static str {
        match self {
            Self::Head => "equipment_slot_head",
            Self::Body => "equipment_slot_body",
            Self::Arms => "equipment_slot_arms",
            Self::Legs => "equipment_slot_legs",
            Self::Feet => "equipment_slot_feet",
            Self::Weapon => "equipment_slot_weapon",
            Self::Offhand => "equipment_slot_offhand",
            Self::Backpack => "equipment_slot_backpack",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Body => "Body",
            Self::Arms => "Arms",
            Self::Legs => "Legs",
            Self::Feet => "Feet",
            Self::Weapon => "Weapon",
            Self::Offhand => "Offhand",
            Self::Backpack => "Backpack",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "head" => Ok(Self::Head),
            "body" => Ok(Self::Body),
            "arms" => Ok(Self::Arms),
            "legs" => Ok(Self::Legs),
            "feet" => Ok(Self::Feet),
            "weapon" => Ok(Self::Weapon),
            "offhand" => Ok(Self::Offhand),
            "backpack" => Ok(Self::Backpack),
            other => Err(format!("unknown equipment slot `{other}`")),
        }
    }

    pub fn parse_list(value: &str) -> Result<Vec<Self>, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        trimmed.split(',').map(|part| Self::parse(part)).collect()
    }
}
