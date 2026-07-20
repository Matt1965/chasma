//! Dirty/cadence-driven Building Intent Propagation step (SA5).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::operation::OperationCatalog;
use crate::world::settlement::response::ResponseCatalog;
use crate::world::settlement::SettlementId;
use crate::world::WorldData;

use super::propagate::{propagate_settlement_intent_to_buildings, PropagationContext};

/// Rebuild when not dirty if this many ticks elapsed.
pub const INTENT_PROPAGATION_CADENCE_TICKS: u64 = 30;

/// Propagate SettlementIntent → BuildingOperationPolicy when intent changes or dirty.
///
/// Never creates tasks, haul requests, construction, or mutates BuildingOperationState.
pub fn step_building_intent_propagation(
    world: &mut WorldData,
    response_catalog: &ResponseCatalog,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    simulation_tick: u64,
) -> u32 {
    let settlement_ids: Vec<SettlementId> = world
        .settlement_state_store()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    let mut propagated = 0u32;
    for settlement_id in settlement_ids {
        let Some(intent_plan) = world.settlement_intent_store().get(settlement_id).cloned() else {
            continue;
        };

        let due_by_dirty = world
            .building_intent_propagation_store()
            .is_dirty(settlement_id);
        let due_by_intent_change = world
            .building_intent_propagation_store()
            .get(settlement_id)
            .map(|prev| prev.source_intent_tick != intent_plan.planned_tick)
            .unwrap_or(true);
        let due_by_cadence = match world.building_intent_propagation_store().get(settlement_id) {
            None => true,
            Some(prev) => {
                simulation_tick.saturating_sub(prev.propagated_tick)
                    >= INTENT_PROPAGATION_CADENCE_TICKS
            }
        };
        if !due_by_dirty && !due_by_intent_change && !due_by_cadence {
            continue;
        }

        let mut ctx = PropagationContext {
            world,
            building_catalog,
            operation_catalog,
            response_catalog,
            settlement_id,
            intent_plan: &intent_plan,
            simulation_tick,
        };
        let report = propagate_settlement_intent_to_buildings(&mut ctx);
        world.building_intent_propagation_store_mut().insert(report);
        propagated += 1;
    }
    propagated
}

/// Force-propagate one settlement (tests / tools).
pub fn propagate_building_intent_now(
    world: &mut WorldData,
    response_catalog: &ResponseCatalog,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    settlement_id: SettlementId,
    simulation_tick: u64,
) {
    let Some(intent_plan) = world.settlement_intent_store().get(settlement_id).cloned() else {
        return;
    };
    let mut ctx = PropagationContext {
        world,
        building_catalog,
        operation_catalog,
        response_catalog,
        settlement_id,
        intent_plan: &intent_plan,
        simulation_tick,
    };
    let report = propagate_settlement_intent_to_buildings(&mut ctx);
    world.building_intent_propagation_store_mut().insert(report);
}
