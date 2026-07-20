//! Dirty/cadence construction planning step (SA9).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::settlement::response::ResponseType;
use crate::world::settlement::SettlementId;
use crate::world::{DoodadCatalog, FootprintCatalog, UnitCatalog, WorldData};

use super::catalog::{BuildingConstructionCostCatalog, ConstructionResponseCatalog};
use super::evaluate::{plan_construction_for_settlement, ConstructionPlanningContext};

pub const CONSTRUCTION_PLANNING_CADENCE_TICKS: u64 = 45;

/// Mark settlements dirty for construction planning when ConstructBuilding intents exist.
pub fn mark_construction_planning_dirty_from_intents(world: &mut WorldData) {
    let ids: Vec<SettlementId> = world
        .settlement_state_store()
        .iter()
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        let has_construct = world.settlement_intent_store().get(id).is_some_and(|plan| {
            plan.intents
                .iter()
                .any(|i| i.response_type == ResponseType::ConstructBuilding)
        });
        if has_construct {
            world
                .construction_planning_report_store_mut()
                .mark_dirty(id);
        }
    }
}

/// Run construction planning after intent arbitration and before strategic task generation.
pub fn step_settlement_construction_planning(
    world: &mut WorldData,
    response_catalog: &ConstructionResponseCatalog,
    cost_catalog: &BuildingConstructionCostCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    unit_catalog: &UnitCatalog,
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
        let due_by_dirty = world
            .construction_planning_report_store()
            .is_dirty(settlement_id)
            || world
                .settlement_state_store()
                .get(settlement_id)
                .is_some_and(|s| s.planner.dirty);
        let due_by_cadence = match world.construction_planning_report_store().get(settlement_id) {
            None => true,
            Some(prev) => {
                simulation_tick.saturating_sub(prev.planned_tick)
                    >= CONSTRUCTION_PLANNING_CADENCE_TICKS
            }
        };
        if !due_by_dirty && !due_by_cadence {
            continue;
        }

        let mut ctx = ConstructionPlanningContext {
            world,
            response_catalog,
            cost_catalog,
            building_catalog,
            footprint_catalog,
            doodad_catalog,
            unit_catalog,
            inventory_ctx,
            simulation_tick,
        };
        let _ = plan_construction_for_settlement(&mut ctx, settlement_id);
        evaluated += 1;
    }
    evaluated
}

pub fn plan_construction_now(
    world: &mut WorldData,
    response_catalog: &ConstructionResponseCatalog,
    cost_catalog: &BuildingConstructionCostCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    unit_catalog: &UnitCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    settlement_id: SettlementId,
    simulation_tick: u64,
) {
    let mut ctx = ConstructionPlanningContext {
        world,
        response_catalog,
        cost_catalog,
        building_catalog,
        footprint_catalog,
        doodad_catalog,
        unit_catalog,
        inventory_ctx,
        simulation_tick,
    };
    let _ = plan_construction_for_settlement(&mut ctx, settlement_id);
}
