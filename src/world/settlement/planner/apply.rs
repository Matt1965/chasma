//! Apply planner intent to building operation policies (EP9).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::{BuildingOperationPolicy, ControlSource, RepeatMode};
use crate::world::operation::{OperationCatalog, validate_operation_selection};
use crate::world::{BuildingId, WorldData};

use super::types::PlannerBuildingDecision;

/// Apply planner decisions to `BuildingOperationPolicy` only — never runtime state (EP9).
///
/// Skips buildings currently owned by SA5 Building Intent Propagation so SettlementIntent
/// remains the strategic authority for those policies.
pub fn apply_planner_decisions(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    decisions: &[PlannerBuildingDecision],
) {
    for decision in decisions {
        if crate::world::settlement::building_owned_by_intent_propagation(
            world,
            decision.building_id,
        ) {
            continue;
        }
        let Some(record) = world.get_building(decision.building_id) else {
            continue;
        };
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if validate_operation_selection(
            definition,
            decision.building_id,
            operation_catalog,
            &decision.operation_id,
        )
        .is_err()
        {
            continue;
        }

        let store = world.building_production_store_mut();
        let policy = store.get_policy_mut(decision.building_id);
        apply_decision_to_policy(policy, decision);
        if policy.selected_operation.is_none() {
            policy.selected_operation = Some(decision.operation_id.clone());
        }
    }
}

fn apply_decision_to_policy(policy: &mut BuildingOperationPolicy, decision: &PlannerBuildingDecision) {
    policy.planner_managed = true;
    policy.control_source = ControlSource::AIControlled;
    policy.enabled = decision.enabled;
    policy.paused = false;
    policy.selected_operation = Some(decision.operation_id.clone());
    policy.repeat_mode = RepeatMode::Continuous;
    policy.priority = decision.priority;
}

/// Disable planner-managed production for buildings not in the decision set (EP9).
pub fn disable_unselected_planner_buildings(
    world: &mut WorldData,
    settlement_building_ids: &[BuildingId],
    active_building_ids: &[BuildingId],
) {
    let active: std::collections::BTreeSet<_> = active_building_ids.iter().copied().collect();
    for building_id in settlement_building_ids {
        if active.contains(building_id) {
            continue;
        }
        if crate::world::settlement::building_owned_by_intent_propagation(world, *building_id) {
            continue;
        }
        let store = world.building_production_store_mut();
        let Some(policy) = store.get_policy(*building_id) else {
            continue;
        };
        if !policy.planner_managed || policy.control_source == ControlSource::PlayerControlled {
            continue;
        }
        let policy = store.get_policy_mut(*building_id);
        policy.enabled = false;
    }
}
