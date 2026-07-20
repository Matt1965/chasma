//! Building ↔ operation catalog binding tests (EP3).

use crate::world::building::category::BuildingCategoryId;
use crate::world::building::footprint::FootprintSpec;
use crate::world::operation::{
    OperationCatalog, OperationDefinitionId, OperationSelectionError,
    validate_building_definition_operations, validate_building_operation_bindings,
    validate_operation_selection,
};
use crate::world::{
    BuildingCatalog, BuildingDefinition, BuildingDefinitionId, BuildingRenderKey, ItemCatalog,
    TerrainFieldCatalog,
};

fn field_catalog() -> TerrainFieldCatalog {
    TerrainFieldCatalog::default()
}
fn sample_building(supported: Vec<OperationDefinitionId>) -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("test_mine"),
        "Test Mine",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        100,
        30.0,
        FootprintSpec::Circle { radius_meters: 2.0 },
        30.0,
        true,
    )
    .with_supported_operations(supported)
    .with_default_operation_id(OperationDefinitionId::new("mine_iron"))
}

#[test]
fn default_operation_must_be_supported() {
    let operations = OperationCatalog::default();
    let items = ItemCatalog::default();
    let building = BuildingDefinition::new(
        BuildingDefinitionId::new("bad"),
        "Bad",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        100,
        30.0,
        FootprintSpec::Circle { radius_meters: 2.0 },
        30.0,
        true,
    )
    .with_supported_operations(vec![OperationDefinitionId::new("mine_stone")])
    .with_default_operation_id(OperationDefinitionId::new("mine_iron"));
    let issues = validate_building_definition_operations(
        &building,
        &operations,
        &items,
        &field_catalog(),
    );
    assert!(issues.iter().any(|issue| issue.message().contains("default operation")));
}

#[test]
fn unsupported_operation_reference_fails_validation() {
    let operations = OperationCatalog::default();
    let items = ItemCatalog::default();
    let building = sample_building(vec![OperationDefinitionId::new("missing_op")]);
    let issues = validate_building_definition_operations(
        &building,
        &operations,
        &items,
        &field_catalog(),
    );
    assert!(issues.iter().any(|issue| issue.message().contains("unknown operation")));
}

#[test]
fn duplicate_supported_operations_fail_validation() {
    let operations = OperationCatalog::default();
    let items = ItemCatalog::default();
    let op = OperationDefinitionId::new("mine_iron");
    let building = sample_building(vec![op.clone(), op]);
    let issues = validate_building_definition_operations(
        &building,
        &operations,
        &items,
        &field_catalog(),
    );
    assert!(issues.iter().any(|issue| issue.message().contains("more than once")));
}

#[test]
fn building_catalog_cross_validation_reports_building_and_operation() {
    let categories = crate::world::BuildingCategoryCatalog::default();
    let operations = OperationCatalog::default();
    let items = ItemCatalog::default();
    let building = sample_building(vec![OperationDefinitionId::new("mine_iron")]);
    let buildings =
        BuildingCatalog::from_definitions(vec![building], &categories).unwrap();
    let issues = validate_building_operation_bindings(
        &buildings,
        &operations,
        &items,
        &field_catalog(),
    );
    assert!(issues.is_empty());
}

#[test]
fn resolved_default_uses_single_supported_operation() {
    let building = BuildingDefinition::new(
        BuildingDefinitionId::new("solo"),
        "Solo",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        100,
        30.0,
        FootprintSpec::Circle { radius_meters: 2.0 },
        30.0,
        true,
    )
    .with_supported_operations(vec![OperationDefinitionId::new("mine_iron")]);
    assert_eq!(
        building.resolved_default_operation().map(|id| id.as_str().to_string()),
        Some("mine_iron".into())
    );
}

#[test]
fn resolved_default_is_none_when_multiple_supported_without_authoring() {
    let building = BuildingDefinition::new(
        BuildingDefinitionId::new("multi"),
        "Multi",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        100,
        30.0,
        FootprintSpec::Circle { radius_meters: 2.0 },
        30.0,
        true,
    )
    .with_supported_operations(vec![
        OperationDefinitionId::new("mine_iron"),
        OperationDefinitionId::new("mine_stone"),
    ]);
    assert!(building.resolved_default_operation().is_none());
}

#[test]
fn runtime_selection_rejects_missing_operation_definition() {
    let operations = OperationCatalog::default();
    let building = sample_building(vec![OperationDefinitionId::new("mine_iron")]);
    let err = validate_operation_selection(
        &building,
        crate::world::BuildingId::new(1),
        &operations,
        &OperationDefinitionId::new("removed_operation"),
    );
    assert!(matches!(err, Err(OperationSelectionError::MissingDefinition(_))));
}

#[test]
fn runtime_selection_rejects_unsupported_operation() {
    let operations = OperationCatalog::default();
    let building = sample_building(vec![OperationDefinitionId::new("mine_stone")]);
    let err = validate_operation_selection(
        &building,
        crate::world::BuildingId::new(1),
        &operations,
        &OperationDefinitionId::new("mine_iron"),
    );
    assert!(matches!(
        err,
        Err(OperationSelectionError::UnsupportedByBuilding { .. })
    ));
}

#[test]
fn policy_stores_operation_id_not_definition_metadata() {
    let policy = crate::world::BuildingOperationPolicy {
        selected_operation: Some(OperationDefinitionId::new("mine_iron")),
        ..Default::default()
    };
    let serialized = ron::ser::to_string(&policy).unwrap();
    assert!(serialized.contains("mine_iron"));
    assert!(!serialized.contains("Mine Iron"));
}
