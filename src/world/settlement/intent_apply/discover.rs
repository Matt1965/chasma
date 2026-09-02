//! Capability-based building discovery for SettlementIntent (SA5).
//!
//! Uses authored `supported_operations` / capability requirements — never building names.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::operation::OperationDefinitionId;
use crate::world::settlement::SettlementId;
use crate::world::settlement::response::{CapabilityRequirement, ResponseDefinition};
use crate::world::{BuildingId, BuildingLifecycleState, WorldData};

/// A complete settlement building that can run `operation_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapableBuilding {
    pub building_id: BuildingId,
    pub operation_id: OperationDefinitionId,
}

/// Extract primary production operation requirement from a response definition.
pub fn primary_operation_requirement(
    definition: &ResponseDefinition,
) -> Option<OperationDefinitionId> {
    for req in &definition.capability_requirements {
        if let CapabilityRequirement::SupportingOperation(op) = req {
            return Some(OperationDefinitionId::new(op.clone()));
        }
    }
    None
}

/// Find complete settlement buildings supporting `operation_id`.
///
/// Player-controlled filtering happens at SA5 policy apply (ADR-120 Phase 8).
pub fn discover_capable_buildings(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    settlement_id: SettlementId,
    operation_id: &OperationDefinitionId,
) -> Vec<CapableBuilding> {
    let mut found = Vec::new();
    let building_ids = world
        .settlement_store()
        .buildings_for_settlement(settlement_id);
    for building_id in building_ids {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if record.lifecycle_state != BuildingLifecycleState::Complete {
            continue;
        }
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if !definition.supports_operation(operation_id) {
            continue;
        }
        found.push(CapableBuilding {
            building_id,
            operation_id: operation_id.clone(),
        });
    }
    // Deterministic order by building id.
    found.sort_by_key(|b| b.building_id.raw());
    found
}
