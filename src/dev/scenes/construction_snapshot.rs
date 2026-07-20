//! Scene persistence for construction plans (SA9).

use serde::{Deserialize, Serialize};

use crate::world::{ConstructionPlanSaveState, WorldData};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneConstructionPlanPersistence {
    #[serde(default)]
    pub save_state: ConstructionPlanSaveState,
}

pub fn capture_construction_plan_persistence(
    world: &WorldData,
) -> SceneConstructionPlanPersistence {
    SceneConstructionPlanPersistence {
        save_state: world.construction_plan_store().export_save_state(),
    }
}

pub fn restore_construction_plan_persistence(
    world: &mut WorldData,
    persistence: &SceneConstructionPlanPersistence,
) {
    world
        .construction_plan_store_mut()
        .import_save_state(persistence.save_state.clone());
    // Transient diagnostics never persist.
    world.construction_planning_report_store_mut().clear();
    // Revalidate after load: mark dirty so next planning pass can retry blocked plans.
    world.construction_planning_report_store_mut().mark_all_dirty();
}
