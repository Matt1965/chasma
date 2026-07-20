//! Scene persistence for SettlementState (SA1).

use serde::{Deserialize, Serialize};

use crate::world::{SettlementStateSaveState, WorldData, ensure_settlement_states_for_world};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneSettlementStatePersistence {
    #[serde(default)]
    pub save_state: SettlementStateSaveState,
}

pub fn capture_settlement_state_persistence(world: &WorldData) -> SceneSettlementStatePersistence {
    SceneSettlementStatePersistence {
        save_state: world.settlement_state_store().export_save_state(),
    }
}

pub fn restore_settlement_state_persistence(
    world: &mut WorldData,
    persistence: &SceneSettlementStatePersistence,
) {
    world
        .settlement_state_store_mut()
        .import_save_state(persistence.save_state.clone());
    // Older scenes (pre-v13) may lack state entries — ensure defaults for every record.
    ensure_settlement_states_for_world(world);
    // Rebuild principle: all future planners must recompute from SettlementState.
    world.settlement_state_store_mut().apply_rebuild_principle();
    // SA2–SA8: derived caches never persisted. Active emergencies live in SettlementState.
    let settlement_ids: Vec<_> = world.settlement_state_store().settlement_ids().collect();
    for settlement_id in settlement_ids {
        if let Some(state) = world.settlement_state_store_mut().get_mut(settlement_id) {
            state.emergencies.migrate_legacy_flags(0);
        }
    }
    world.emergency_evaluation_store_mut().clear();
    world.need_evaluation_store_mut().clear();
    world.response_candidate_store_mut().clear();
    world.settlement_intent_store_mut().clear();
    world.building_intent_propagation_store_mut().clear();
    world.strategic_task_generation_store_mut().clear();
    world.worker_assignment_store_mut().clear();
    world.construction_planning_report_store_mut().clear();
}
