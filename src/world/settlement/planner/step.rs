//! Simulation step hook for settlement production planners (EP9).
//!
//! Phase 8: EP9 no longer runs as an independent policy-producing tick stage.
//! Production graph reasoning is invoked by SA5 when applying production intents.

use crate::world::WorldData;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::operation::OperationCatalog;

/// Retained for API compatibility. EP9 policy writes were removed in Phase 8.
pub fn step_settlement_production_planners(
    _world: &mut WorldData,
    _building_catalog: &BuildingCatalog,
    _operation_catalog: &OperationCatalog,
    _inventory_ctx: &InventoryCatalogCtx<'_>,
    _simulation_tick: u64,
) -> u32 {
    0
}

/// Mark planner dirty when settlement inventory or buildings change (EP9).
///
/// Also dirties SettlementState planner lifecycle (SA1) — no evaluation runs here.
pub fn mark_settlement_planner_dirty(world: &mut WorldData, building_id: crate::world::BuildingId) {
    crate::world::settlement::mark_settlement_state_dirty_for_building(world, building_id);
}
