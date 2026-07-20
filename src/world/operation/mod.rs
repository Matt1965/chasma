//! Production operation catalog (EP3).

pub mod catalog;

pub use catalog::{
    BuildingOperationBindingError, OperationCatalog, OperationCatalogError, OperationCategory,
    OperationDefinition, OperationDefinitionId, OperationEffectKind, OperationId,
    OperationInputDefinition, OperationIoValidationError, OperationOutputDefinition,
    OperationPowerRequirementRef, OperationSelectionError, OperationSkillRequirementRef,
    OperationTerrainRequirementRef, OperationToolRequirementRef, starter_definitions,
    validate_building_definition_operations, validate_building_operation_bindings,
    validate_operation_selection,
};

#[cfg(any(test, feature = "dev"))]
pub use catalog::test_workbench_operation;
