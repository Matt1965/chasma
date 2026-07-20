//! Dirty/cadence-driven settlement response arbitration (SA4).

use crate::world::settlement::response::ResponseCatalog;
use crate::world::settlement::SettlementId;
use crate::world::WorldData;

use super::arbitrate::{arbitrate_settlement_intent, ArbitrationContext};

/// Rebuild when not dirty if this many ticks elapsed (fallback cadence).
pub const INTENT_ARBITRATION_CADENCE_TICKS: u64 = 30;

/// Replan SettlementIntent when candidates change, settlement dirty, or cadence expires.
///
/// Never mutates buildings, tasks, inventories, or BuildingOperationPolicy.
pub fn step_settlement_response_arbitration(
    world: &mut WorldData,
    response_catalog: &ResponseCatalog,
    simulation_tick: u64,
) -> u32 {
    let settlement_ids: Vec<SettlementId> = world
        .settlement_state_store()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    let mut planned = 0u32;
    for settlement_id in settlement_ids {
        let Some(candidates) = world.response_candidate_store().get(settlement_id).cloned() else {
            continue;
        };
        let Some(need_eval) = world.need_evaluation_store().get(settlement_id).cloned() else {
            continue;
        };

        let due_by_dirty = world.settlement_intent_store().is_dirty(settlement_id);
        let due_by_response_change = world
            .settlement_intent_store()
            .get(settlement_id)
            .map(|prev| prev.source_response_tick != candidates.evaluated_tick)
            .unwrap_or(true);
        let due_by_need_change = world
            .settlement_intent_store()
            .get(settlement_id)
            .map(|prev| prev.source_need_tick != need_eval.evaluated_tick)
            .unwrap_or(true);
        let due_by_cadence = match world.settlement_intent_store().get(settlement_id) {
            None => true,
            Some(prev) => {
                simulation_tick.saturating_sub(prev.planned_tick) >= INTENT_ARBITRATION_CADENCE_TICKS
            }
        };
        if !due_by_dirty && !due_by_response_change && !due_by_need_change && !due_by_cadence {
            continue;
        }

        let Some(state) = world.settlement_state_store().get(settlement_id).cloned() else {
            continue;
        };

        let ctx = ArbitrationContext {
            world,
            response_catalog,
            settlement_id,
            state: &state,
            need_evaluation: &need_eval,
            candidates: &candidates,
            simulation_tick,
        };
        let plan = arbitrate_settlement_intent(&ctx);
        world.settlement_intent_store_mut().insert(plan);
        // SA5/SA6: intent change invalidates policy propagation + strategic tasks.
        world
            .building_intent_propagation_store_mut()
            .mark_dirty(settlement_id);
        world
            .strategic_task_generation_store_mut()
            .mark_dirty(settlement_id);
        planned += 1;
    }
    planned
}

/// Force-arbitrate one settlement (tests / tools).
pub fn arbitrate_settlement_intent_now(
    world: &mut WorldData,
    response_catalog: &ResponseCatalog,
    settlement_id: SettlementId,
    simulation_tick: u64,
) {
    let Some(candidates) = world.response_candidate_store().get(settlement_id).cloned() else {
        return;
    };
    let Some(need_eval) = world.need_evaluation_store().get(settlement_id).cloned() else {
        return;
    };
    let Some(state) = world.settlement_state_store().get(settlement_id).cloned() else {
        return;
    };
    let ctx = ArbitrationContext {
        world,
        response_catalog,
        settlement_id,
        state: &state,
        need_evaluation: &need_eval,
        candidates: &candidates,
        simulation_tick,
    };
    let plan = arbitrate_settlement_intent(&ctx);
    world.settlement_intent_store_mut().insert(plan);
    world
        .building_intent_propagation_store_mut()
        .mark_dirty(settlement_id);
    world
        .strategic_task_generation_store_mut()
        .mark_dirty(settlement_id);
}
