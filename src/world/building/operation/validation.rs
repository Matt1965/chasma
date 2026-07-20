//! Production runtime validation (EP2/EP3).

use crate::world::BuildingId;
use crate::world::WorldData;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::{
    OperationDefinitionId, OperationLifecycle, PRODUCTION_PROGRESS_ONE_UNIT, RepeatMode,
};
use crate::world::operation::OperationCatalog;

/// Actionable production validation issue (EP2/EP3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionValidationIssue {
    OrphanedState { building_id: BuildingId },
    OrphanedPolicy { building_id: BuildingId },
    InvalidRepeatCount { building_id: BuildingId, count: u32 },
    ProgressOverflow { building_id: BuildingId, progress: u64 },
    ImpossibleLifecycle {
        building_id: BuildingId,
        lifecycle: OperationLifecycle,
        policy_enabled: bool,
        policy_paused: bool,
    },
    MissingOperationDefinition {
        building_id: BuildingId,
        operation_id: OperationDefinitionId,
    },
    UnsupportedOperation {
        building_id: BuildingId,
        operation_id: OperationDefinitionId,
    },
}

impl ProductionValidationIssue {
    pub fn message(&self) -> String {
        match self {
            Self::OrphanedState { building_id } => format!(
                "production state exists for missing building #{}",
                building_id.raw()
            ),
            Self::OrphanedPolicy { building_id } => format!(
                "production policy exists for missing building #{}",
                building_id.raw()
            ),
            Self::InvalidRepeatCount { building_id, count } => format!(
                "building #{} has invalid repeat count {count}",
                building_id.raw()
            ),
            Self::ProgressOverflow { building_id, progress } => format!(
                "building #{} progress {progress} exceeds fixed-point threshold",
                building_id.raw()
            ),
            Self::ImpossibleLifecycle {
                building_id,
                lifecycle,
                policy_enabled,
                policy_paused,
            } => format!(
                "building #{} lifecycle {:?} conflicts with policy enabled={policy_enabled} paused={policy_paused}",
                building_id.raw(),
                lifecycle
            ),
            Self::MissingOperationDefinition {
                building_id,
                operation_id,
            } => format!(
                "building #{} selected operation `{}` is missing from the catalog",
                building_id.raw(),
                operation_id.as_str()
            ),
            Self::UnsupportedOperation {
                building_id,
                operation_id,
            } => format!(
                "building #{} selected operation `{}` is not supported by its definition",
                building_id.raw(),
                operation_id.as_str()
            ),
        }
    }
}

/// Validate authoritative production runtime invariants (EP2).
pub fn validate_production_runtime(world: &WorldData) -> Vec<ProductionValidationIssue> {
    validate_production_runtime_with_catalogs(world, None, None)
}

/// Validate production runtime including catalog resolution (EP3).
pub fn validate_production_runtime_with_catalogs(
    world: &WorldData,
    building_catalog: Option<&BuildingCatalog>,
    operation_catalog: Option<&OperationCatalog>,
) -> Vec<ProductionValidationIssue> {
    let store = world.building_production_store();
    let mut issues = Vec::new();

    for building_id in store.building_ids() {
        if world.get_building(building_id).is_none() {
            if store.get_state(building_id).is_some() {
                issues.push(ProductionValidationIssue::OrphanedState { building_id });
            }
            if store.get_policy(building_id).is_some() {
                issues.push(ProductionValidationIssue::OrphanedPolicy { building_id });
            }
            continue;
        }

        if let Some(policy) = store.get_policy(building_id) {
            if let RepeatMode::Count(0) = policy.repeat_mode {
                issues.push(ProductionValidationIssue::InvalidRepeatCount {
                    building_id,
                    count: 0,
                });
            }
            if let Some(operation_id) = policy.selected_operation.as_ref() {
                if let Some(operation_catalog) = operation_catalog {
                    if operation_catalog.get(operation_id).is_none() {
                        issues.push(ProductionValidationIssue::MissingOperationDefinition {
                            building_id,
                            operation_id: operation_id.clone(),
                        });
                    }
                }
                if let (Some(building_catalog), Some(record)) = (
                    building_catalog,
                    world.get_building(building_id),
                ) {
                    if let Some(definition) = building_catalog.get(&record.definition_id) {
                        if !definition.supports_operation(operation_id) {
                            issues.push(ProductionValidationIssue::UnsupportedOperation {
                                building_id,
                                operation_id: operation_id.clone(),
                            });
                        }
                    }
                }
            }
        }

        if let Some(state) = store.get_state(building_id) {
            if state.progress.value() >= PRODUCTION_PROGRESS_ONE_UNIT {
                issues.push(ProductionValidationIssue::ProgressOverflow {
                    building_id,
                    progress: state.progress.value(),
                });
            }

            if let Some(policy) = store.get_policy(building_id) {
                if !policy.enabled && state.lifecycle == OperationLifecycle::Running {
                    issues.push(ProductionValidationIssue::ImpossibleLifecycle {
                        building_id,
                        lifecycle: state.lifecycle,
                        policy_enabled: policy.enabled,
                        policy_paused: policy.paused,
                    });
                }
                if policy.paused && state.lifecycle == OperationLifecycle::Running {
                    issues.push(ProductionValidationIssue::ImpossibleLifecycle {
                        building_id,
                        lifecycle: state.lifecycle,
                        policy_enabled: policy.enabled,
                        policy_paused: policy.paused,
                    });
                }
            }
        }
    }

    issues
}

/// Production stepping is worker-task-driven — no global building scan required (EP2).
pub const PRODUCTION_STEPPING_MODEL: &str = "worker-task-driven";
