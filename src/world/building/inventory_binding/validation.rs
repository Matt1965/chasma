//! Building inventory binding validation (EP4).

use std::collections::HashSet;

use crate::world::BuildingCatalog;
use crate::world::BuildingId;
use crate::world::InventoryProfileCatalog;
use crate::world::WorldData;
use crate::world::building::catalog::BuildingDefinition;
use crate::world::operation::{OperationDefinition, OperationOutputDefinition};

use super::binding_id::BuildingInventoryBindingId;
use super::definition::BuildingInventoryBindingDefinition;
use super::role::BuildingInventoryRole;
use super::store::BuildingInventoryBindingStore;

/// Building inventory binding validation issue (EP4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingInventoryBindingValidationIssue {
    DuplicateBindingId {
        building: crate::world::BuildingDefinitionId,
        binding_id: BuildingInventoryBindingId,
    },
    MultipleDefaultBindings {
        building: crate::world::BuildingDefinitionId,
    },
    InvalidDefaultBinding {
        building: crate::world::BuildingDefinitionId,
        binding_id: BuildingInventoryBindingId,
    },
    MissingProfile {
        building: crate::world::BuildingDefinitionId,
        binding_id: BuildingInventoryBindingId,
        profile_id: crate::world::InventoryProfileId,
    },
    MissingRuntimeBinding {
        building_id: BuildingId,
        binding_id: BuildingInventoryBindingId,
    },
    MissingInventoryRecord {
        building_id: BuildingId,
        binding_id: BuildingInventoryBindingId,
        inventory_id: crate::world::InventoryId,
    },
    DuplicateInventoryClaim {
        building_id: BuildingId,
        inventory_id: crate::world::InventoryId,
    },
    OrphanedBuildingInventory {
        building_id: BuildingId,
        inventory_id: crate::world::InventoryId,
    },
    OperationUnknownBinding {
        operation_id: crate::world::OperationDefinitionId,
        binding_id: BuildingInventoryBindingId,
    },
    OperationInputRoleMismatch {
        operation_id: crate::world::OperationDefinitionId,
        binding_id: BuildingInventoryBindingId,
        role: BuildingInventoryRole,
    },
    OperationOutputRoleMismatch {
        operation_id: crate::world::OperationDefinitionId,
        binding_id: BuildingInventoryBindingId,
        role: BuildingInventoryRole,
    },
    RuntimeBindingRoleMismatch {
        building_id: BuildingId,
        binding_id: BuildingInventoryBindingId,
        expected_role: BuildingInventoryRole,
        actual_role: BuildingInventoryRole,
    },
}

impl BuildingInventoryBindingValidationIssue {
    pub fn message(&self) -> String {
        match self {
            Self::DuplicateBindingId {
                building,
                binding_id,
            } => format!(
                "building `{}` defines binding `{binding_id}` more than once",
                building.as_str()
            ),
            Self::MultipleDefaultBindings { building } => format!(
                "building `{}` defines more than one default inventory binding",
                building.as_str()
            ),
            Self::InvalidDefaultBinding {
                building,
                binding_id,
            } => format!(
                "building `{}` default binding `{binding_id}` is not supported",
                building.as_str()
            ),
            Self::MissingProfile {
                building,
                binding_id,
                profile_id,
            } => format!(
                "building `{}` binding `{binding_id}` references unknown profile `{}`",
                building.as_str(),
                profile_id.as_str()
            ),
            Self::MissingRuntimeBinding {
                building_id,
                binding_id,
            } => format!(
                "building #{building_id} missing runtime binding `{binding_id}`",
                building_id = building_id.raw()
            ),
            Self::MissingInventoryRecord {
                building_id,
                binding_id,
                inventory_id,
            } => format!(
                "building #{building_id} binding `{binding_id}` references missing inventory #{inventory_id}",
                building_id = building_id.raw(),
                inventory_id = inventory_id.raw()
            ),
            Self::DuplicateInventoryClaim {
                building_id,
                inventory_id,
            } => format!(
                "building #{building_id} claims inventory #{inventory_id} more than once",
                building_id = building_id.raw(),
                inventory_id = inventory_id.raw()
            ),
            Self::OrphanedBuildingInventory {
                building_id,
                inventory_id,
            } => format!(
                "orphaned inventory #{inventory_id} for building #{building_id}",
                building_id = building_id.raw(),
                inventory_id = inventory_id.raw()
            ),
            Self::OperationUnknownBinding {
                operation_id,
                binding_id,
            } => format!(
                "operation `{}` references unknown binding `{binding_id}`",
                operation_id.as_str()
            ),
            Self::OperationInputRoleMismatch {
                operation_id,
                binding_id,
                role,
            } => format!(
                "operation `{}` input binding `{binding_id}` targets incompatible role `{}`",
                operation_id.as_str(),
                role.label()
            ),
            Self::OperationOutputRoleMismatch {
                operation_id,
                binding_id,
                role,
            } => format!(
                "operation `{}` output binding `{binding_id}` targets incompatible role `{}`",
                operation_id.as_str(),
                role.label()
            ),
            Self::RuntimeBindingRoleMismatch {
                building_id,
                binding_id,
                expected_role,
                actual_role,
            } => format!(
                "building #{} binding `{}` expected role `{}` but has `{}`",
                building_id.raw(),
                binding_id,
                expected_role.label(),
                actual_role.label(),
            ),
        }
    }
}

pub fn effective_inventory_binding_definitions(
    building: &BuildingDefinition,
) -> Vec<BuildingInventoryBindingDefinition> {
    if !building.inventory_bindings.is_empty() {
        return building.inventory_bindings.clone();
    }
    building
        .inventory_profile_id
        .clone()
        .map(BuildingInventoryBindingDefinition::legacy_primary)
        .into_iter()
        .collect()
}

pub fn validate_building_definition_inventory_bindings(
    building: &BuildingDefinition,
    profile_catalog: &InventoryProfileCatalog,
) -> Vec<BuildingInventoryBindingValidationIssue> {
    let mut issues = Vec::new();
    let bindings = effective_inventory_binding_definitions(building);
    let mut seen_ids = HashSet::new();
    let mut default_count = 0usize;

    for binding in &bindings {
        if !seen_ids.insert(binding.binding_id.clone()) {
            issues.push(BuildingInventoryBindingValidationIssue::DuplicateBindingId {
                building: building.id.clone(),
                binding_id: binding.binding_id.clone(),
            });
        }
        if binding.is_default {
            default_count += 1;
        }
        if profile_catalog.get(&binding.profile_id).is_none() {
            issues.push(BuildingInventoryBindingValidationIssue::MissingProfile {
                building: building.id.clone(),
                binding_id: binding.binding_id.clone(),
                profile_id: binding.profile_id.clone(),
            });
        }
    }

    if default_count > 1 {
        issues.push(
            BuildingInventoryBindingValidationIssue::MultipleDefaultBindings {
                building: building.id.clone(),
            },
        );
    }

    if let Some(default_id) = &building.default_inventory_binding_id {
        if !bindings.iter().any(|binding| &binding.binding_id == default_id) {
            issues.push(BuildingInventoryBindingValidationIssue::InvalidDefaultBinding {
                building: building.id.clone(),
                binding_id: default_id.clone(),
            });
        }
    }

    issues
}

pub fn validate_building_catalog_inventory_bindings(
    building_catalog: &BuildingCatalog,
    profile_catalog: &InventoryProfileCatalog,
) -> Vec<BuildingInventoryBindingValidationIssue> {
    building_catalog
        .definitions()
        .iter()
        .flat_map(|definition| {
            validate_building_definition_inventory_bindings(definition, profile_catalog)
        })
        .collect()
}

pub fn validate_operation_inventory_bindings(
    operation: &OperationDefinition,
    building: &BuildingDefinition,
) -> Vec<BuildingInventoryBindingValidationIssue> {
    let mut issues = Vec::new();
    let bindings = effective_inventory_binding_bindings_map(building);

    for input in &operation.inputs {
        if let Some(binding_id) = &input.source_binding {
            validate_operation_input_binding(operation, building, binding_id, &bindings, &mut issues);
        }
    }
    for output in &operation.outputs {
        if let OperationOutputDefinition::Item {
            destination_binding: Some(binding_id),
            ..
        } = output
        {
            validate_operation_output_binding(
                operation,
                building,
                binding_id,
                &bindings,
                &mut issues,
            );
        }
    }
    issues
}

fn effective_inventory_binding_bindings_map(
    building: &BuildingDefinition,
) -> std::collections::HashMap<BuildingInventoryBindingId, BuildingInventoryRole> {
    effective_inventory_binding_definitions(building)
        .into_iter()
        .map(|binding| (binding.binding_id, binding.role))
        .collect()
}

fn validate_operation_input_binding(
    operation: &OperationDefinition,
    building: &BuildingDefinition,
    binding_id: &BuildingInventoryBindingId,
    bindings: &std::collections::HashMap<BuildingInventoryBindingId, BuildingInventoryRole>,
    issues: &mut Vec<BuildingInventoryBindingValidationIssue>,
) {
    let Some(role) = bindings.get(binding_id) else {
        issues.push(BuildingInventoryBindingValidationIssue::OperationUnknownBinding {
            operation_id: operation.id.clone(),
            binding_id: binding_id.clone(),
        });
        return;
    };
    if !role.accepts_operation_input() {
        issues.push(
            BuildingInventoryBindingValidationIssue::OperationInputRoleMismatch {
                operation_id: operation.id.clone(),
                binding_id: binding_id.clone(),
                role: *role,
            },
        );
    }
    let _ = building;
}

fn validate_operation_output_binding(
    operation: &OperationDefinition,
    building: &BuildingDefinition,
    binding_id: &BuildingInventoryBindingId,
    bindings: &std::collections::HashMap<BuildingInventoryBindingId, BuildingInventoryRole>,
    issues: &mut Vec<BuildingInventoryBindingValidationIssue>,
) {
    let Some(role) = bindings.get(binding_id) else {
        issues.push(BuildingInventoryBindingValidationIssue::OperationUnknownBinding {
            operation_id: operation.id.clone(),
            binding_id: binding_id.clone(),
        });
        return;
    };
    if !role.accepts_operation_output() {
        issues.push(
            BuildingInventoryBindingValidationIssue::OperationOutputRoleMismatch {
                operation_id: operation.id.clone(),
                binding_id: binding_id.clone(),
                role: *role,
            },
        );
    }
    let _ = building;
}

pub fn validate_building_runtime_inventory_bindings(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
) -> Vec<BuildingInventoryBindingValidationIssue> {
    let mut issues = Vec::new();
    let Some(record) = world.get_building(building_id) else {
        return issues;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return issues;
    };
    let authored = effective_inventory_binding_definitions(definition);
    let runtime = world.building_inventory_binding_store().get(building_id);

    if authored.is_empty() {
        return issues;
    }

    let runtime_set = match runtime {
        Some(set) => set,
        None => {
            for binding in &authored {
                issues.push(BuildingInventoryBindingValidationIssue::MissingRuntimeBinding {
                    building_id,
                    binding_id: binding.binding_id.clone(),
                });
            }
            return issues;
        }
    };

    let mut claimed = HashSet::new();
    for authored_binding in &authored {
        let Some(runtime_binding) = runtime_set.get(&authored_binding.binding_id) else {
            issues.push(BuildingInventoryBindingValidationIssue::MissingRuntimeBinding {
                building_id,
                binding_id: authored_binding.binding_id.clone(),
            });
            continue;
        };
        if runtime_binding.role != authored_binding.role {
            issues.push(
                BuildingInventoryBindingValidationIssue::RuntimeBindingRoleMismatch {
                    building_id,
                    binding_id: authored_binding.binding_id.clone(),
                    expected_role: authored_binding.role,
                    actual_role: runtime_binding.role,
                },
            );
        }
        if !claimed.insert(runtime_binding.inventory_id) {
            issues.push(BuildingInventoryBindingValidationIssue::DuplicateInventoryClaim {
                building_id,
                inventory_id: runtime_binding.inventory_id,
            });
        }
        if world
            .inventory_store()
            .get(runtime_binding.inventory_id)
            .is_none()
        {
            issues.push(BuildingInventoryBindingValidationIssue::MissingInventoryRecord {
                building_id,
                binding_id: authored_binding.binding_id.clone(),
                inventory_id: runtime_binding.inventory_id,
            });
        }
    }

    issues
}

pub fn validate_world_building_inventory_bindings(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
) -> Vec<BuildingInventoryBindingValidationIssue> {
    let mut issues = Vec::new();
    for building_id in world.sorted_building_ids() {
        issues.extend(validate_building_runtime_inventory_bindings(
            world,
            building_catalog,
            building_id,
        ));
    }

    let store = world.building_inventory_binding_store();
    for building_id in store.building_ids() {
        if world.get_building(building_id).is_none() {
            if let Some(set) = store.get(building_id) {
                for binding in set.bindings() {
                    issues.push(BuildingInventoryBindingValidationIssue::OrphanedBuildingInventory {
                        building_id,
                        inventory_id: binding.inventory_id,
                    });
                }
            }
        }
    }

    issues
}

/// Validate selected operation inventory bindings for a building (EP4).
pub fn validate_selected_operation_inventory_bindings(
    operation: &OperationDefinition,
    building: &BuildingDefinition,
    building_id: BuildingId,
    binding_store: &BuildingInventoryBindingStore,
) -> Result<(), BuildingInventoryBindingValidationIssue> {
    for issue in validate_operation_inventory_bindings(operation, building) {
        return Err(issue);
    }

    for input in &operation.inputs {
        if let Some(binding_id) = &input.source_binding {
            if binding_store
                .resolve_inventory(building_id, binding_id)
                .is_none()
            {
                return Err(BuildingInventoryBindingValidationIssue::MissingRuntimeBinding {
                    building_id,
                    binding_id: binding_id.clone(),
                });
            }
        }
    }
    for output in &operation.outputs {
        if let OperationOutputDefinition::Item {
            destination_binding: Some(binding_id),
            ..
        } = output
        {
            if binding_store
                .resolve_inventory(building_id, binding_id)
                .is_none()
            {
                return Err(BuildingInventoryBindingValidationIssue::MissingRuntimeBinding {
                    building_id,
                    binding_id: binding_id.clone(),
                });
            }
        }
    }
    Ok(())
}
