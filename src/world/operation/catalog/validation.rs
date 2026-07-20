//! Operation catalog validation (EP3).

use crate::world::BuildingCatalog;
use crate::world::BuildingDefinition;
use crate::world::BuildingDefinitionId;
use crate::world::ItemCatalog;

use super::definition_id::OperationDefinitionId;
use super::registry::OperationCatalog;

/// Catalog build/authoring validation error (EP3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationCatalogError {
    DuplicateId(OperationDefinitionId),
    InvalidDefinition {
        operation_id: OperationDefinitionId,
        reason: String,
    },
}

impl std::fmt::Display for OperationCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate operation id `{id}`"),
            Self::InvalidDefinition { operation_id, reason } => {
                write!(f, "operation `{}`: {reason}", operation_id.as_str())
            }
        }
    }
}

/// Building ↔ operation compatibility validation error (EP3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingOperationBindingError {
    UnknownOperation {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
    },
    DuplicateSupportedOperation {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
    },
    UnsupportedDefaultOperation {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
    },
    InvalidMaxWorkers {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
        max_workers: u32,
    },
    InvalidBaseLabor {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
        base_labor: u32,
    },
    UnknownInputItem {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
        item_id: crate::world::ItemDefinitionId,
    },
    UnknownOutputItem {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
        item_id: crate::world::ItemDefinitionId,
    },
    UnknownTerrainField {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
        field_id: crate::world::TerrainFieldId,
    },
    MalformedInput {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
        reason: String,
    },
    MalformedOutput {
        building_id: BuildingDefinitionId,
        operation_id: OperationDefinitionId,
        reason: String,
    },
}

impl BuildingOperationBindingError {
    pub fn message(&self) -> String {
        match self {
            Self::UnknownOperation {
                building_id,
                operation_id,
            } => format!(
                "building `{}` references unknown operation `{}`",
                building_id.as_str(),
                operation_id.as_str()
            ),
            Self::DuplicateSupportedOperation {
                building_id,
                operation_id,
            } => format!(
                "building `{}` lists operation `{}` more than once",
                building_id.as_str(),
                operation_id.as_str()
            ),
            Self::UnsupportedDefaultOperation {
                building_id,
                operation_id,
            } => format!(
                "building `{}` default operation `{}` is not supported",
                building_id.as_str(),
                operation_id.as_str()
            ),
            Self::InvalidMaxWorkers {
                building_id,
                operation_id,
                max_workers,
            } => format!(
                "building `{}` operation `{}` has invalid max_workers {max_workers}",
                building_id.as_str(),
                operation_id.as_str()
            ),
            Self::InvalidBaseLabor {
                building_id,
                operation_id,
                base_labor,
            } => format!(
                "building `{}` operation `{}` has invalid base_labor {base_labor}",
                building_id.as_str(),
                operation_id.as_str()
            ),
            Self::UnknownInputItem {
                building_id,
                operation_id,
                item_id,
            } => format!(
                "building `{}` operation `{}` references unknown input item `{}`",
                building_id.as_str(),
                operation_id.as_str(),
                item_id.as_str()
            ),
            Self::UnknownOutputItem {
                building_id,
                operation_id,
                item_id,
            } => format!(
                "building `{}` operation `{}` references unknown output item `{}`",
                building_id.as_str(),
                operation_id.as_str(),
                item_id.as_str()
            ),
            Self::UnknownTerrainField {
                building_id,
                operation_id,
                field_id,
            } => format!(
                "building `{}` operation `{}` references unknown terrain field `{}`",
                building_id.as_str(),
                operation_id.as_str(),
                field_id.as_str()
            ),
            Self::MalformedInput {
                building_id,
                operation_id,
                reason,
            } => format!(
                "building `{}` operation `{}` input: {reason}",
                building_id.as_str(),
                operation_id.as_str()
            ),
            Self::MalformedOutput {
                building_id,
                operation_id,
                reason,
            } => format!(
                "building `{}` operation `{}` output: {reason}",
                building_id.as_str(),
                operation_id.as_str()
            ),
        }
    }
}

/// Runtime production selection validation error (EP3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationSelectionError {
    MissingDefinition(OperationDefinitionId),
    UnsupportedByBuilding {
        building_id: crate::world::BuildingId,
        operation_id: OperationDefinitionId,
    },
}

impl std::fmt::Display for OperationSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDefinition(id) => write!(f, "operation `{id}` is not in the catalog"),
            Self::UnsupportedByBuilding {
                building_id,
                operation_id,
            } => write!(
                f,
                "building #{building_id} does not support operation `{operation_id}`",
                building_id = building_id.raw(),
                operation_id = operation_id.as_str()
            ),
        }
    }
}

pub fn validate_building_operation_bindings(
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    item_catalog: &ItemCatalog,
    field_catalog: &crate::world::TerrainFieldCatalog,
) -> Vec<BuildingOperationBindingError> {
    let mut issues = Vec::new();
    for definition in building_catalog.definitions() {
        issues.extend(validate_building_definition_operations(
            definition,
            operation_catalog,
            item_catalog,
            field_catalog,
        ));
    }
    issues
}

pub fn validate_building_definition_operations(
    building: &BuildingDefinition,
    operation_catalog: &OperationCatalog,
    item_catalog: &ItemCatalog,
    field_catalog: &crate::world::TerrainFieldCatalog,
) -> Vec<BuildingOperationBindingError> {
    let mut issues = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for operation_id in &building.supported_operations {
        if !seen.insert(operation_id.clone()) {
            issues.push(BuildingOperationBindingError::DuplicateSupportedOperation {
                building_id: building.id.clone(),
                operation_id: operation_id.clone(),
            });
            continue;
        }
        let Some(operation) = operation_catalog.get(operation_id) else {
            issues.push(BuildingOperationBindingError::UnknownOperation {
                building_id: building.id.clone(),
                operation_id: operation_id.clone(),
            });
            continue;
        };
        if operation.max_workers == 0 {
            issues.push(BuildingOperationBindingError::InvalidMaxWorkers {
                building_id: building.id.clone(),
                operation_id: operation_id.clone(),
                max_workers: operation.max_workers,
            });
        }
        if operation.base_labor == 0 {
            issues.push(BuildingOperationBindingError::InvalidBaseLabor {
                building_id: building.id.clone(),
                operation_id: operation_id.clone(),
                base_labor: operation.base_labor,
            });
        }
        for input in &operation.inputs {
            if item_catalog.get(&input.item_id).is_none() {
                issues.push(BuildingOperationBindingError::UnknownInputItem {
                    building_id: building.id.clone(),
                    operation_id: operation_id.clone(),
                    item_id: input.item_id.clone(),
                });
            }
            if let Err(err) = input.validate() {
                issues.push(BuildingOperationBindingError::MalformedInput {
                    building_id: building.id.clone(),
                    operation_id: operation_id.clone(),
                    reason: format!("{err:?}"),
                });
            }
        }
        for output in &operation.outputs {
            if let crate::world::OperationOutputDefinition::Item { item_id, .. } = output {
                if item_catalog.get(item_id).is_none() {
                    issues.push(BuildingOperationBindingError::UnknownOutputItem {
                        building_id: building.id.clone(),
                        operation_id: operation_id.clone(),
                        item_id: item_id.clone(),
                    });
                }
            }
            if let Err(err) = output.validate() {
                issues.push(BuildingOperationBindingError::MalformedOutput {
                    building_id: building.id.clone(),
                    operation_id: operation_id.clone(),
                    reason: format!("{err:?}"),
                });
            }
        }
        for terrain in &operation.terrain_requirements {
            if field_catalog.get(&terrain.field_id).is_none() {
                issues.push(BuildingOperationBindingError::UnknownTerrainField {
                    building_id: building.id.clone(),
                    operation_id: operation_id.clone(),
                    field_id: terrain.field_id.clone(),
                });
            }
            if let Err(err) = terrain.validate() {
                issues.push(BuildingOperationBindingError::MalformedOutput {
                    building_id: building.id.clone(),
                    operation_id: operation_id.clone(),
                    reason: format!("invalid terrain requirement: {err:?}"),
                });
            }
        }
    }

    if let Some(default_id) = &building.default_operation_id {
        if !building.supports_operation(default_id) {
            issues.push(BuildingOperationBindingError::UnsupportedDefaultOperation {
                building_id: building.id.clone(),
                operation_id: default_id.clone(),
            });
        } else if operation_catalog.get(default_id).is_none() {
            issues.push(BuildingOperationBindingError::UnknownOperation {
                building_id: building.id.clone(),
                operation_id: default_id.clone(),
            });
        }
    }

    issues
}

pub fn validate_operation_selection(
    building: &BuildingDefinition,
    building_id: crate::world::BuildingId,
    operation_catalog: &OperationCatalog,
    operation_id: &OperationDefinitionId,
) -> Result<(), OperationSelectionError> {
    if operation_catalog.get(operation_id).is_none() {
        return Err(OperationSelectionError::MissingDefinition(operation_id.clone()));
    }
    if !building.supports_operation(operation_id) {
        return Err(OperationSelectionError::UnsupportedByBuilding {
            building_id,
            operation_id: operation_id.clone(),
        });
    }
    Ok(())
}
