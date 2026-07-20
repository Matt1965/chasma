//! Scene persistence for hauling logistics runtime (EP7).

use serde::{Deserialize, Serialize};

use crate::world::{LogisticsSaveState, WorldData};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneLogisticsPersistence {
    #[serde(default)]
    pub save_state: LogisticsSaveState,
}

pub fn capture_logistics_persistence(world: &WorldData) -> SceneLogisticsPersistence {
    SceneLogisticsPersistence {
        save_state: crate::world::export_logistics_save_state(world),
    }
}

pub fn restore_logistics_persistence(world: &mut WorldData, persistence: &SceneLogisticsPersistence) {
    crate::world::import_logistics_save_state(world, persistence.save_state.clone());
}
