use bevy::prelude::*;

use crate::world::corpse::CorpseId;
use crate::world::{BuildingId, UnitId};

/// Authoritative owner of an inventory container (ADR-088 I2, ADR-089 I3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Default)]
pub enum InventoryOwnerRef {
    #[default]
    Detached,
    Unit(UnitId),
    Building(BuildingId),
    Corpse(CorpseId),
}

impl InventoryOwnerRef {
    pub fn is_detached(&self) -> bool {
        matches!(self, Self::Detached)
    }
}
