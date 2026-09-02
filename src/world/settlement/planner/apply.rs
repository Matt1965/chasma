//! Test/dev helpers for applying EP9 recommendations through SA5 (Phase 8).
//!
//! Production policy writes are owned by SA5. This module routes recommendations to the
//! authoritative SA5 apply path for tests and legacy dev tooling.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::operation::OperationCatalog;
use crate::world::settlement::intent_apply::{
    sa5_apply_policy_decision, sa5_disable_unselected_ai_buildings,
};
use crate::world::{BuildingId, WorldData};

use super::types::PlannerBuildingDecision;

/// Apply planner recommendations through the SA5 policy writer (tests/dev only).
pub fn apply_production_recommendations_for_tests(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    settlement_building_ids: &[BuildingId],
    decisions: &[PlannerBuildingDecision],
) {
    let active: Vec<BuildingId> = decisions
        .iter()
        .filter(|decision| decision.enabled)
        .map(|decision| decision.building_id)
        .collect();
    sa5_disable_unselected_ai_buildings(world, settlement_building_ids, &active);
    for decision in decisions {
        let _ = sa5_apply_policy_decision(world, building_catalog, operation_catalog, decision);
    }
}

/// Disable AI-controlled buildings not in the active decision set (test helper).
pub fn disable_unselected_planner_buildings(
    world: &mut WorldData,
    settlement_building_ids: &[BuildingId],
    active_building_ids: &[BuildingId],
) {
    sa5_disable_unselected_ai_buildings(world, settlement_building_ids, active_building_ids);
}
