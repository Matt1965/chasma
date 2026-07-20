//! SettlementState dirty invalidation (SA1). No evaluation — flags only.
//! Also dirties SA2–SA4 caches and EP9 production planner.

use crate::world::settlement::SettlementId;
use crate::world::WorldData;

/// Mark settlement runtime dirty. Also dirties need/response/intent caches + EP9 planner.
///
/// Does not run planners or need evaluation. SA steps treat this as a reevaluation hint.
pub fn mark_settlement_state_dirty(world: &mut WorldData, settlement_id: SettlementId) {
    world.settlement_state_store_mut().mark_dirty(settlement_id);
    world.emergency_evaluation_store_mut().mark_dirty(settlement_id);
    world.need_evaluation_store_mut().mark_dirty(settlement_id);
    world.response_candidate_store_mut().mark_dirty(settlement_id);
    world.settlement_intent_store_mut().mark_dirty(settlement_id);
    world
        .building_intent_propagation_store_mut()
        .mark_dirty(settlement_id);
    world
        .strategic_task_generation_store_mut()
        .mark_dirty(settlement_id);
    world.production_planner_store_mut().mark_dirty(settlement_id);
}

/// Mark dirty for the settlement that owns `building_id`, if any.
pub fn mark_settlement_state_dirty_for_building(
    world: &mut WorldData,
    building_id: crate::world::BuildingId,
) {
    let Some(settlement_id) = world.settlement_store().settlement_for_building(building_id) else {
        return;
    };
    mark_settlement_state_dirty(world, settlement_id);
}

/// Mark all settlement states dirty (e.g. after bulk restore before rebuild).
pub fn mark_all_settlement_states_dirty(world: &mut WorldData) {
    world.settlement_state_store_mut().mark_all_dirty();
    world.emergency_evaluation_store_mut().mark_all_dirty();
    world.need_evaluation_store_mut().mark_all_dirty();
    world.response_candidate_store_mut().mark_all_dirty();
    world.settlement_intent_store_mut().mark_all_dirty();
    world
        .building_intent_propagation_store_mut()
        .mark_all_dirty();
    world.strategic_task_generation_store_mut().mark_all_dirty();
    world.production_planner_store_mut().mark_all_dirty();
}
