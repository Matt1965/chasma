//! Dirty/cadence emergency evaluation step (SA8).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::ItemCatalog;
use crate::world::settlement::SettlementId;
use crate::world::{mark_settlement_state_dirty, WorldData};

use super::catalog::EmergencyCatalog;
use super::evaluate::{evaluate_settlement_emergencies, EmergencyEvalContext};

pub const EMERGENCY_EVAL_CADENCE_TICKS: u64 = 30;

/// Evaluate emergencies before need evaluation so pressures see updated state.
pub fn step_settlement_emergency_evaluation(
    world: &mut WorldData,
    catalog: &EmergencyCatalog,
    building_catalog: &BuildingCatalog,
    item_catalog: &ItemCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    simulation_tick: u64,
) -> u32 {
    let settlement_ids: Vec<SettlementId> = world
        .settlement_state_store()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    let mut evaluated = 0u32;
    for settlement_id in settlement_ids {
        let due_by_dirty = world.emergency_evaluation_store().is_dirty(settlement_id)
            || world
                .settlement_state_store()
                .get(settlement_id)
                .is_some_and(|s| s.planner.dirty);
        let due_by_cadence = match world.emergency_evaluation_store().get(settlement_id) {
            None => true,
            Some(prev) => {
                simulation_tick.saturating_sub(prev.evaluated_tick) >= EMERGENCY_EVAL_CADENCE_TICKS
            }
        };
        if !due_by_dirty && !due_by_cadence {
            continue;
        }

        let Some(mut state) = world.settlement_state_store().get(settlement_id).cloned() else {
            continue;
        };
        let before = state.emergencies.instances.clone();

        let ctx = EmergencyEvalContext {
            world,
            building_catalog,
            item_catalog,
            inventory_ctx,
            settlement_id,
            simulation_tick,
        };
        let report = evaluate_settlement_emergencies(&ctx, catalog, &mut state);

        let changed = before != state.emergencies.instances;
        world.settlement_state_store_mut().insert(state);
        world.emergency_evaluation_store_mut().insert(report);
        if changed {
            mark_settlement_state_dirty(world, settlement_id);
        }
        evaluated += 1;
    }
    evaluated
}

pub fn evaluate_settlement_emergencies_now(
    world: &mut WorldData,
    catalog: &EmergencyCatalog,
    building_catalog: &BuildingCatalog,
    item_catalog: &ItemCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    settlement_id: SettlementId,
    simulation_tick: u64,
) {
    let Some(mut state) = world.settlement_state_store().get(settlement_id).cloned() else {
        return;
    };
    let ctx = EmergencyEvalContext {
        world,
        building_catalog,
        item_catalog,
        inventory_ctx,
        settlement_id,
        simulation_tick,
    };
    let report = evaluate_settlement_emergencies(&ctx, catalog, &mut state);
    world.settlement_state_store_mut().insert(state);
    world.emergency_evaluation_store_mut().insert(report);
    mark_settlement_state_dirty(world, settlement_id);
}
