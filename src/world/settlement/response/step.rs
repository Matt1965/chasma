//! Cadence/dirty-driven response discovery step (SA3).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::NeedCatalog;
use crate::world::settlement::SettlementId;
use crate::world::WorldData;

use super::catalog::ResponseCatalog;
use super::discover::{discover_settlement_responses, ResponseDiscoveryContext};

/// Rebuild when not dirty if this many ticks elapsed (fallback cadence).
pub const RESPONSE_DISCOVERY_CADENCE_TICKS: u64 = 30;

/// Rebuild CandidateResponses when needs changed, settlement dirty, or cadence expires.
///
/// Never generates tasks, mutates buildings/policies/inventories, or selects a response.
pub fn step_settlement_response_discovery(
    world: &mut WorldData,
    need_catalog: &NeedCatalog,
    response_catalog: &ResponseCatalog,
    emergency_catalog: &EmergencyCatalog,
    building_catalog: &BuildingCatalog,
    simulation_tick: u64,
) -> u32 {
    let settlement_ids: Vec<SettlementId> = world
        .settlement_state_store()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    let mut discovered = 0u32;
    for settlement_id in settlement_ids {
        let Some(need_eval) = world.need_evaluation_store().get(settlement_id).cloned() else {
            continue;
        };

        let due_by_dirty = world.response_candidate_store().is_dirty(settlement_id);
        let due_by_need_change = world
            .response_candidate_store()
            .get(settlement_id)
            .map(|prev| prev.source_need_tick != need_eval.evaluated_tick)
            .unwrap_or(true);
        let due_by_cadence = match world.response_candidate_store().get(settlement_id) {
            None => true,
            Some(prev) => {
                simulation_tick.saturating_sub(prev.evaluated_tick)
                    >= RESPONSE_DISCOVERY_CADENCE_TICKS
            }
        };
        if !due_by_dirty && !due_by_need_change && !due_by_cadence {
            continue;
        }

        let Some(state) = world.settlement_state_store().get(settlement_id).cloned() else {
            continue;
        };

        let ctx = ResponseDiscoveryContext {
            world,
            building_catalog,
            need_catalog,
            response_catalog,
            emergency_catalog,
            settlement_id,
            state: &state,
            need_evaluation: &need_eval,
            simulation_tick,
        };
        let result = discover_settlement_responses(&ctx);
        world.response_candidate_store_mut().insert(result);
        // SA4: candidate change invalidates SettlementIntent.
        world.settlement_intent_store_mut().mark_dirty(settlement_id);
        discovered += 1;
    }
    discovered
}

/// Force-discover for one settlement (tests / tools).
pub fn discover_settlement_responses_now(
    world: &mut WorldData,
    need_catalog: &NeedCatalog,
    response_catalog: &ResponseCatalog,
    emergency_catalog: &EmergencyCatalog,
    building_catalog: &BuildingCatalog,
    settlement_id: SettlementId,
    simulation_tick: u64,
) {
    let Some(need_eval) = world.need_evaluation_store().get(settlement_id).cloned() else {
        return;
    };
    let Some(state) = world.settlement_state_store().get(settlement_id).cloned() else {
        return;
    };
    let ctx = ResponseDiscoveryContext {
        world,
        building_catalog,
        need_catalog,
        response_catalog,
        emergency_catalog,
        settlement_id,
        state: &state,
        need_evaluation: &need_eval,
        simulation_tick,
    };
    let result = discover_settlement_responses(&ctx);
    world.response_candidate_store_mut().insert(result);
    world.settlement_intent_store_mut().mark_dirty(settlement_id);
}
