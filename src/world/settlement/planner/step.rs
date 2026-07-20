//! Simulation step hook for settlement production planners (EP9).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::operation::OperationCatalog;
use crate::world::settlement::SettlementId;
use crate::world::WorldData;

use super::plan::execute_settlement_replan;

/// Advance settlement production planners when dirty or interval elapsed (EP9).
pub fn step_settlement_production_planners(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    simulation_tick: u64,
) -> u32 {
    let settlement_ids: Vec<SettlementId> = world.settlement_store().sorted_settlement_ids();
    let mut replanned = 0u32;
    for settlement_id in settlement_ids {
        let should_replan = world
            .production_planner_store()
            .get(settlement_id)
            .map(|planner| {
                if !planner.enabled {
                    false
                } else if planner.dirty {
                    true
                } else {
                    simulation_tick.saturating_sub(planner.last_plan_tick)
                        >= planner.replan_interval_ticks
                }
            })
            .unwrap_or(false);
        if !should_replan {
            continue;
        }
        let planner_snapshot = world
            .production_planner_store()
            .get(settlement_id)
            .cloned();
        let Some(mut planner) = planner_snapshot else {
            continue;
        };
        execute_settlement_replan(
            world,
            building_catalog,
            operation_catalog,
            inventory_ctx,
            settlement_id,
            &mut planner,
            simulation_tick,
        );
        let stored = world.production_planner_store_mut().get_mut(settlement_id);
        stored.last_diagnostics = planner.last_diagnostics;
        stored.last_plan_tick = planner.last_plan_tick;
        stored.dirty = planner.dirty;
        replanned += 1;
    }
    replanned
}

/// Mark planner dirty when settlement inventory or buildings change (EP9).
///
/// Also dirties SettlementState planner lifecycle (SA1) — no evaluation runs here.
pub fn mark_settlement_planner_dirty(world: &mut WorldData, building_id: crate::world::BuildingId) {
    crate::world::settlement::mark_settlement_state_dirty_for_building(world, building_id);
}
