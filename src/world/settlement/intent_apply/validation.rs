//! Building Intent Propagation validation (SA5).

use std::collections::BTreeSet;

use super::report::BuildingIntentPropagationReport;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::operation::OperationCatalog;
use crate::world::WorldData;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropagationValidationError {
    UnknownBuilding(u64),
    MissingCapability {
        building_id: u64,
        operation: String,
    },
    InvalidOperation {
        building_id: u64,
        operation: String,
    },
    DuplicateAssignment(u64),
    ConflictingOwnership {
        building_id: u64,
        detail: String,
    },
}

impl std::fmt::Display for PropagationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownBuilding(id) => write!(f, "unknown building #{id}"),
            Self::MissingCapability {
                building_id,
                operation,
            } => write!(
                f,
                "building #{building_id} missing capability for `{operation}`"
            ),
            Self::InvalidOperation {
                building_id,
                operation,
            } => write!(
                f,
                "building #{building_id} invalid operation `{operation}`"
            ),
            Self::DuplicateAssignment(id) => write!(f, "duplicate assignment for building #{id}"),
            Self::ConflictingOwnership {
                building_id,
                detail,
            } => write!(f, "building #{building_id} ownership conflict: {detail}"),
        }
    }
}

pub fn validate_propagation_report(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    report: &BuildingIntentPropagationReport,
) -> Vec<PropagationValidationError> {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();

    for assignment in &report.assignments {
        let id = assignment.building_id.raw();
        if !seen.insert(id) {
            errors.push(PropagationValidationError::DuplicateAssignment(id));
        }
        let Some(record) = world.get_building(assignment.building_id) else {
            errors.push(PropagationValidationError::UnknownBuilding(id));
            continue;
        };
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            errors.push(PropagationValidationError::UnknownBuilding(id));
            continue;
        };
        if let Some(op) = &assignment.selected_operation {
            if !definition.supports_operation(op) {
                errors.push(PropagationValidationError::MissingCapability {
                    building_id: id,
                    operation: op.as_str().to_string(),
                });
            }
            if operation_catalog.get(op).is_none() {
                errors.push(PropagationValidationError::InvalidOperation {
                    building_id: id,
                    operation: op.as_str().to_string(),
                });
            }
        }
        if let Some(policy) = world
            .building_production_store()
            .get_policy(assignment.building_id)
        {
            if policy.control_source
                == crate::world::building::operation::ControlSource::PlayerControlled
                && policy.planner_managed
            {
                errors.push(PropagationValidationError::ConflictingOwnership {
                    building_id: id,
                    detail: "player-reclaimed but SA5-assigned".into(),
                });
            }
        }
    }

    errors
}
