mod category;
mod definition;
mod definition_id;
mod io;
mod registry;
mod starter;
mod validation;

#[cfg(test)]
mod binding_tests;

pub use category::OperationCategory;
pub use definition::OperationDefinition;
pub use definition_id::{OperationDefinitionId, OperationId};
pub use io::{
    OperationEffectKind, OperationInputDefinition, OperationIoValidationError,
    OperationOutputDefinition, OperationPowerRequirementRef, OperationSkillRequirementRef,
    OperationTerrainRequirementRef, OperationToolRequirementRef,
};
pub use registry::OperationCatalog;
pub use starter::starter_definitions;
pub use validation::{
    BuildingOperationBindingError, OperationCatalogError, OperationSelectionError,
    validate_building_definition_operations, validate_building_operation_bindings,
    validate_operation_selection,
};

#[cfg(any(test, feature = "dev"))]
pub use starter::test_workbench_operation;
