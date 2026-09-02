//! Self-maintenance activity tracking (ADR-134 / ADR-071 seam).

use bevy::prelude::*;

use crate::world::{BuildingId, InventoryId, WorldPosition};

use super::nutrition::HungerStage;

/// Where a unit is obtaining food.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub enum FoodSourceRef {
    OwnInventory {
        inventory_id: InventoryId,
    },
    SettlementStorage {
        inventory_id: InventoryId,
        building_id: BuildingId,
    },
}

/// Autonomous self-maintenance activity for one unit.
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub enum SelfMaintenanceActivity {
    #[default]
    None,
    SeekingFood {
        source: FoodSourceRef,
        destination: WorldPosition,
        stage: HungerStage,
    },
    Eating {
        source: FoodSourceRef,
        stage: HungerStage,
    },
}

/// Persistent self-maintenance state on [`super::super::record::UnitRecord`].
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub struct UnitSelfMaintenanceState {
    pub activity: SelfMaintenanceActivity,
}

impl UnitSelfMaintenanceState {
    pub fn clear(&mut self) {
        self.activity = SelfMaintenanceActivity::None;
    }

    pub fn is_seeking_or_eating(&self) -> bool {
        !matches!(self.activity, SelfMaintenanceActivity::None)
    }
}
