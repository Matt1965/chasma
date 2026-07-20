//! Capacity-gap estimation for construction planning (SA9).

use crate::world::building::catalog::{BuildingCatalog, BuildingDefinitionId};
use crate::world::settlement::SettlementId;
use crate::world::{BuildingLifecycleState, WorldData};

use super::catalog::{ConstructionCapabilityKind, ConstructionResponseMapping};
use super::plan::ConstructionPlanStatus;
use super::store::ConstructionPlanStore;

#[derive(Debug, Clone, PartialEq)]
pub struct CapacityGapEstimate {
    pub existing_capable: u32,
    pub planned_capable: u32,
    pub target_capacity: u32,
    pub additional_needed: u32,
    pub notes: Vec<String>,
}

/// Estimate whether additional construction capacity is required for a mapping.
pub fn estimate_capacity_gap(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    plan_store: &ConstructionPlanStore,
    settlement_id: SettlementId,
    mapping: &ConstructionResponseMapping,
) -> CapacityGapEstimate {
    let mut notes = Vec::new();
    if !mapping.creates_new_capacity {
        notes.push("mapping does not create new capacity".into());
        return CapacityGapEstimate {
            existing_capable: 0,
            planned_capable: 0,
            target_capacity: mapping.target_capacity,
            additional_needed: 0,
            notes,
        };
    }

    let building_ids = world
        .settlement_store()
        .buildings_for_settlement(settlement_id);
    let mut existing_capable = 0u32;
    for building_id in building_ids {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if matches!(
            record.lifecycle_state,
            BuildingLifecycleState::Ruins | BuildingLifecycleState::Destroyed
        ) {
            continue;
        }
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if building_satisfies_mapping(definition, mapping) {
            existing_capable = existing_capable.saturating_add(1);
        }
    }

    let mut planned_capable = 0u32;
    for plan in plan_store.plans_for_settlement(settlement_id) {
        if !plan.status.is_active() || plan.status == ConstructionPlanStatus::Proposed {
            continue;
        }
        if plan.required_capability != mapping.capability_key {
            continue;
        }
        // Count reserved/incomplete plan capacity toward the gap.
        if plan.reserved_building_id.is_some()
            || matches!(
                plan.status,
                ConstructionPlanStatus::SiteSearch
                    | ConstructionPlanStatus::AwaitingApproval
                    | ConstructionPlanStatus::AwaitingMaterials
                    | ConstructionPlanStatus::Ready
                    | ConstructionPlanStatus::InProgress
                    | ConstructionPlanStatus::Blocked
            )
        {
            planned_capable = planned_capable.saturating_add(1);
        }
    }

    let covered = existing_capable.saturating_add(planned_capable);
    let additional_needed = mapping.target_capacity.saturating_sub(covered);
    notes.push(format!(
        "existing={} planned={} target={} need={}",
        existing_capable, planned_capable, mapping.target_capacity, additional_needed
    ));

    CapacityGapEstimate {
        existing_capable,
        planned_capable,
        target_capacity: mapping.target_capacity,
        additional_needed,
        notes,
    }
}

pub fn building_satisfies_mapping(
    definition: &crate::world::building::catalog::BuildingDefinition,
    mapping: &ConstructionResponseMapping,
) -> bool {
    if !definition.enabled {
        return false;
    }
    if !mapping.eligible_building_ids.is_empty()
        && !mapping
            .eligible_building_ids
            .contains(&definition.id)
    {
        // Explicit allow-list restricts; category/operation filters still apply when empty.
        if matches!(
            mapping.capability_kind,
            ConstructionCapabilityKind::ExplicitAllowList
        ) {
            return false;
        }
    }
    match &mapping.capability_kind {
        ConstructionCapabilityKind::SupportingOperation(op) => {
            definition.supported_operations.contains(op)
        }
        ConstructionCapabilityKind::BuildingCategory(category) => {
            &definition.category_id == category
        }
        ConstructionCapabilityKind::ExplicitAllowList => mapping
            .eligible_building_ids
            .contains(&definition.id),
    }
}

pub fn fulfillment_key(
    settlement_id: SettlementId,
    mapping: &ConstructionResponseMapping,
    building_definition_id: &BuildingDefinitionId,
) -> String {
    format!(
        "{}:{}:{}:{}",
        settlement_id.raw(),
        mapping.response_id.as_str(),
        mapping.capability_key,
        building_definition_id.as_str()
    )
}
