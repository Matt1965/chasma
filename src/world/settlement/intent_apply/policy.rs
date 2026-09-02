//! SA5 authoritative BuildingOperationPolicy writes (ADR-120 Phase 8).
//!
//! EP9 recommends; this module applies. Player-controlled buildings are never mutated.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::{BuildingOperationPolicy, ControlSource, RepeatMode};
use crate::world::operation::{OperationCatalog, validate_operation_selection};
use crate::world::settlement::planner::PlannerBuildingDecision;
use crate::world::{BuildingId, WorldData};

/// Whether SA5 may mutate this building's production policy.
pub fn can_sa5_mutate_policy(policy: &BuildingOperationPolicy) -> bool {
    policy.control_source != ControlSource::PlayerControlled
}

/// Apply one planner recommendation through the sole AI policy writer (SA5).
pub fn sa5_apply_policy_decision(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    decision: &PlannerBuildingDecision,
) -> bool {
    let Some(record) = world.get_building(decision.building_id) else {
        return false;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return false;
    };
    if validate_operation_selection(
        definition,
        decision.building_id,
        operation_catalog,
        &decision.operation_id,
    )
    .is_err()
    {
        return false;
    }

    let store = world.building_production_store_mut();
    store.ensure_policy_for_building(decision.building_id, definition, operation_catalog);
    let policy = store.get_policy_mut(decision.building_id);
    if !can_sa5_mutate_policy(policy) {
        return false;
    }
    apply_decision_to_policy(policy, decision);
    true
}

/// Disable AI-controlled production for settlement buildings not selected this propagation pass.
pub fn sa5_disable_unselected_ai_buildings(
    world: &mut WorldData,
    settlement_building_ids: &[BuildingId],
    active_building_ids: &[BuildingId],
) {
    let active: std::collections::BTreeSet<_> = active_building_ids.iter().copied().collect();
    for building_id in settlement_building_ids {
        if active.contains(building_id) {
            continue;
        }
        let store = world.building_production_store_mut();
        let Some(policy) = store.get_policy(*building_id) else {
            continue;
        };
        if !can_sa5_mutate_policy(policy) || !policy.enabled {
            continue;
        }
        let policy = store.get_policy_mut(*building_id);
        policy.enabled = false;
    }
}

fn apply_decision_to_policy(
    policy: &mut BuildingOperationPolicy,
    decision: &PlannerBuildingDecision,
) {
    policy.planner_managed = true;
    policy.control_source = ControlSource::AIControlled;
    policy.enabled = decision.enabled;
    policy.paused = false;
    policy.selected_operation = Some(decision.operation_id.clone());
    policy.repeat_mode = RepeatMode::Continuous;
    policy.priority = decision.priority;
}
