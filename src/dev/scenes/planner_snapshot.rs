//! Scene persistence for settlement production planners (EP9).

use serde::{Deserialize, Serialize};

use crate::world::{ProductionPlannerSaveState, WorldData};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneProductionPlannerPersistence {
    #[serde(default)]
    pub save_state: ProductionPlannerSaveState,
}

pub fn capture_production_planner_persistence(
    world: &WorldData,
) -> SceneProductionPlannerPersistence {
    SceneProductionPlannerPersistence {
        save_state: world.production_planner_store().export_save_state(),
    }
}

pub fn restore_production_planner_persistence(
    world: &mut WorldData,
    persistence: &SceneProductionPlannerPersistence,
) {
    world
        .production_planner_store_mut()
        .import_save_state(persistence.save_state.clone());
}
