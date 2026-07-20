//! Scene persistence for building production runtime (EP1).

use serde::{Deserialize, Serialize};

use crate::world::{BuildingProductionSaveState, WorldData};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneProductionPersistence {
    #[serde(default)]
    pub save_state: BuildingProductionSaveState,
}

pub fn capture_production_persistence(world: &WorldData) -> SceneProductionPersistence {
    SceneProductionPersistence {
        save_state: world.building_production_store().export_save_state(),
    }
}

pub fn restore_production_persistence(
    world: &mut WorldData,
    persistence: &SceneProductionPersistence,
) {
    world
        .building_production_store_mut()
        .import_save_state(persistence.save_state.clone());
}
