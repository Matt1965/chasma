//! Cadence-driven need evaluation step (SA2). Read-only over world state; writes snapshots only.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::ItemCatalog;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::SettlementId;
use crate::world::WorldData;

use super::catalog::NeedCatalog;
use super::evaluate::{evaluate_settlement_needs, NeedEvalContext};

/// Default max interval between evaluations when not dirty (ticks).
pub const NEED_EVAL_CADENCE_TICKS: u64 = 30;

/// Recompute need snapshots for settlements that are dirty or past cadence.
///
/// Never generates tasks, mutates buildings/policies/inventories, or writes SettlementState
/// need targets. Does not clear EP9 planner dirty — need dirty lives on NeedEvaluationStore.
pub fn step_settlement_need_evaluation(
    world: &mut WorldData,
    need_catalog: &NeedCatalog,
    building_catalog: &BuildingCatalog,
    item_catalog: &ItemCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    emergency_catalog: &EmergencyCatalog,
    simulation_tick: u64,
) -> u32 {
    let settlement_ids: Vec<SettlementId> = world
        .settlement_state_store()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    let mut evaluated = 0u32;
    for settlement_id in settlement_ids {
        let due_by_dirty = world.need_evaluation_store().is_dirty(settlement_id);
        let due_by_cadence = match world.need_evaluation_store().get(settlement_id) {
            None => true,
            Some(prev) => {
                simulation_tick.saturating_sub(prev.evaluated_tick) >= NEED_EVAL_CADENCE_TICKS
            }
        };
        if !due_by_dirty && !due_by_cadence {
            continue;
        }

        let Some(state) = world.settlement_state_store().get(settlement_id).cloned() else {
            continue;
        };

        let ctx = NeedEvalContext {
            world,
            building_catalog,
            item_catalog,
            inventory_ctx,
            settlement_id,
            state: &state,
            emergency_catalog,
            simulation_tick,
        };
        let evaluation = evaluate_settlement_needs(&ctx, need_catalog);
        world.need_evaluation_store_mut().insert(evaluation);
        // SA3: need snapshot change invalidates response candidates.
        world.response_candidate_store_mut().mark_dirty(settlement_id);
        evaluated += 1;
    }
    evaluated
}

/// Force-evaluate one settlement (tests / tools).
pub fn evaluate_settlement_needs_now(
    world: &mut WorldData,
    need_catalog: &NeedCatalog,
    building_catalog: &BuildingCatalog,
    item_catalog: &ItemCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    emergency_catalog: &EmergencyCatalog,
    settlement_id: SettlementId,
    simulation_tick: u64,
) {
    let Some(state) = world.settlement_state_store().get(settlement_id).cloned() else {
        return;
    };
    let ctx = NeedEvalContext {
        world,
        building_catalog,
        item_catalog,
        inventory_ctx,
        settlement_id,
        state: &state,
        emergency_catalog,
        simulation_tick,
    };
    let evaluation = evaluate_settlement_needs(&ctx, need_catalog);
    world.need_evaluation_store_mut().insert(evaluation);
    world.response_candidate_store_mut().mark_dirty(settlement_id);
}
