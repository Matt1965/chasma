use bevy::prelude::*;

use crate::world::corpse::CorpseId;
use crate::world::equipment::EquipmentSlot;
use crate::world::inventory::ItemInstanceId;
use crate::world::{BuildingId, UnitId};

/// Authoritative owner of an inventory container (ADR-088 I2, ADR-089 I3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Default)]
pub enum InventoryOwnerRef {
    #[default]
    Detached,
    /// Unit personal carried inventory (not equipment slots).
    Unit(UnitId),
    /// One semantic equipment slot inventory for a unit.
    UnitEquipment {
        unit_id: UnitId,
        slot: EquipmentSlot,
    },
    /// Internal inventory owned by a container item instance (e.g. backpack).
    ItemContainer(ItemInstanceId),
    Building(BuildingId),
    Corpse(CorpseId),
    /// One semantic equipment slot inventory for a corpse.
    CorpseEquipment {
        corpse_id: CorpseId,
        slot: EquipmentSlot,
    },
}

impl InventoryOwnerRef {
    pub fn is_detached(&self) -> bool {
        matches!(self, Self::Detached)
    }
}
