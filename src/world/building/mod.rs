pub mod catalog;
pub mod category;
pub mod field_requirement;
pub mod field_response;
pub mod footprint;
pub mod operation;
pub mod operational_efficiency;
pub mod terrain_assessment;

mod asset_pivot;
mod authoring;
mod construction;
pub mod container_access;
mod id;
mod insert;
mod interaction_profile;
mod interior;
pub mod inventory;
pub mod inventory_binding;
pub mod inventory_error;
mod ownership;
mod placement;
mod placement_plan;
mod placement_validation;
mod rebuild;
mod record;
mod restore;
mod source;
mod state;
mod store;
mod transform_edit;
mod vitals;

pub use asset_pivot::{builtin_model_local_offset, effective_model_local_offset};
pub use authoring::{
    BuildingAuthoringError, apply_dev_complete_building_state, create_building,
    create_building_with_inventory, create_dev_complete_building,
    create_dev_complete_building_with_inventory, lookup_building, move_building,
    place_player_building, place_player_building_with_inventory, remove_building,
};
#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_definitions;
pub use catalog::{
    BuildingCatalog, BuildingCatalogError, BuildingDefinition, BuildingDefinitionId,
    BuildingRenderKey,
};
#[cfg(any(test, feature = "dev"))]
pub use category::starter_definitions as starter_building_category_definitions;
pub use category::{
    BuildingCategoryCatalog, BuildingCategoryCatalogError, BuildingCategoryDefinition,
    BuildingCategoryId,
};
pub use construction::{
    BuildingConstructionReport, BuildingConstructionSettings, BuildingLifecycleError,
    BuildingLifecycleEvent, add_building_construction_progress, damage_building, destroy_building,
    heal_building, is_building_operational, set_building_lifecycle_stage,
    step_all_building_construction, transition_to_ruins,
};
pub use container_access::{
    ContainerAccessPolicy, InventoryAccessDenialReason, InventoryAccessResult,
};
pub use field_requirement::{
    BuildingFieldRequirementCatalog, BuildingFieldRequirementCatalogRevision,
    BuildingFieldRequirementDefinition, BuildingFieldRequirementError,
    BuildingFieldRequirementKind, load_building_field_requirement_catalog,
};
pub use field_response::{
    EfficiencyBasisPoints, FieldResponseEvaluationError, FieldResponsePoint,
    FieldResponseProfileCatalog, FieldResponseProfileCatalogRevision,
    FieldResponseProfileDefinition, FieldResponseProfileError, FieldResponseProfileId,
    MAX_EFFICIENCY_BASIS_POINTS, evaluate_field_response, field_value_from_percent,
    field_value_to_percent_display, load_field_response_profile_catalog,
};
pub use footprint::{FootprintSpec, FootprintType};
pub use id::BuildingId;
pub use insert::BuildingInsertError;
pub use interaction_profile::{
    BuildingCapabilities, BuildingInteractionProfile, BuildingInteractionProfileCatalog,
    INTERACTION_WORK_RANGE_METERS, InteractionPointDefinition, interaction_point_world_position,
    starter_interaction_profiles,
};
pub use interior::{
    DoorAccessPolicy, DoorId, DoorRecord, DoorState, DoorStore, InteriorError,
    InteriorProfileCatalog, InteriorProfileId, activate_building_interior, close_door,
    deactivate_building_interior, destroy_door, lock_door, open_door, portal_traversable,
    space_route_for_unit, starter_interior_profiles, try_activate_interior_if_complete,
    try_open_door_at_portal_for_unit, try_open_door_for_unit, two_story_hut_interior_profile,
};
pub use inventory::{
    BuildingInventoryContext, BuildingInventoryRemovalPolicy, attach_inventory_on_building_create,
    building_container_access_policy, building_inventory_operational,
    can_unit_access_building_inventory, can_unit_access_inventory,
    cleanup_building_inventory_on_delete, create_building_inventory,
    finalize_building_inventory_removal, set_building_container_locked, spill_building_inventory,
    spill_position_for_building, validate_building_inventory_links,
    validate_building_inventory_owner,
};
pub use inventory_error::BuildingInventoryError;
pub use inventory_binding::{
    BuildingInventoryBinding, BuildingInventoryBindingDefinition, BuildingInventoryBindingId,
    BuildingInventoryBindingSet, BuildingInventoryBindingStore, BuildingInventoryBindingValidationIssue,
    BuildingInventoryRole, building_inventories_with_role, building_inventory_bindings,
    default_building_inventory_binding, effective_inventory_binding_definitions,
    primary_building_inventory_id, resolve_building_inventory_binding,
    validate_building_catalog_inventory_bindings, validate_building_definition_inventory_bindings,
    validate_building_runtime_inventory_bindings, validate_operation_inventory_bindings,
    validate_selected_operation_inventory_bindings, validate_world_building_inventory_bindings,
};
pub use operation::{
    BASE_OPERATION_PROGRESS_PER_TICK,
    BuildingOperationParams, BuildingOperationPolicy, BuildingOperationSaveState,
    BuildingOperationState, BuildingOperationStore, BuildingProductionSaveState,
    BuildingProductionStore, ControlSource, OperationCompletionReport, OperationDefinitionId,
    OperationError, OperationLifecycle, OperationStepReport, PRODUCTION_PROGRESS_ONE_UNIT,
    ProductionCommandError, ProductionExecutionAssessment, ProductionExecutionFailure,
    ProductionProgress, ProductionValidationIssue, PRODUCTION_STEPPING_MODEL, RepeatMode,
    ResolvedProductionInput, ResolvedProductionOutput, apply_operation_ticks,
    assess_production_execution, expected_ticks_to_complete, execute_production_cycle,
    production_policy, reset_production_progress, scale_progress, set_production_enabled,
    set_production_execution_mode, set_production_paused, set_production_repeat_count,
    set_production_selected_operation, step_workstation_operation,
    validate_production_runtime, validate_production_runtime_with_catalogs,
    cycle_production_selected_operation, workstation_workers_for_building,
};
pub use operational_efficiency::{
    OperationalEfficiencyContext, OperationalEfficiencyError, OperationalEfficiencyReport,
    OperationalLimitingFactor, building_operational_efficiency, combine_output_efficiency,
};
pub use ownership::BuildingOwnership;
pub use placement::BuildingPlacement;
pub use placement_plan::{
    BuildingPlacementPlan, PLACEMENT_QUANTIZE_METERS, anchor_from_terrain_position,
    build_building_placement_plan, building_anchor_render_transform, building_has_model_correction,
    building_model_correction_local_transform, building_model_render_transform,
    building_model_world_transform, ground_and_quantize_building_anchor,
    quantize_placement_anchor_xz, snap_anchor_global_xz,
};
pub use placement_validation::{
    BuildingPlacementConfig, BuildingPlacementContext, BuildingPlacementRejectReason,
    BuildingPlacementValidation, rotation_from_quadrants, validate_building_placement,
    validate_building_transform_placement,
};
pub use rebuild::{BuildingRebuildError, rebuild_building_world_indexes};
pub use record::BuildingRecord;
pub use restore::{BuildingRestoreError, validate_building_for_restore};
pub use source::BuildingSource;
pub use state::{BuildingInteriorState, BuildingLifecycleState, BuildingSpaces, ConstructionState};
pub use store::ChunkBuildingStore;
pub use terrain_assessment::{
    AssessmentRebuildOutcome, AssessmentRebuildReport, BuildingFieldRequirementAssessment,
    BuildingTerrainAssessment, BuildingTerrainAssessmentKey, BuildingTerrainAssessmentStore,
    BuildingTerrainWarning, TerrainAssessmentCatalogs, TerrainAssessmentError,
    assess_building_terrain, assess_building_terrain_at_placement, assessment_revision_fingerprint,
    ensure_building_terrain_assessment, format_coverage_display, format_efficiency_display,
    format_field_average_display, hash_sample_cells, invalidate_buildings_for_changed_fields,
    rebuild_all_building_terrain_assessments, rebuild_building_terrain_assessment,
    resolve_building_field_sample_cells,
};
pub use transform_edit::{
    BuildingTransformCandidate, BuildingTransformCatalogs, BuildingTransformEditError,
    BuildingTransformEditOptions, BuildingTransformEditReport, update_building_transform,
};
pub use vitals::BuildingVitals;
