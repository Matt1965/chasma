pub mod catalog;
pub mod category;
pub mod field_requirement;
pub mod field_response;
pub mod footprint;
pub mod operation;
pub mod operational_efficiency;
pub mod storage_policy;
pub mod terrain_assessment;
pub mod terrain_placement;

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
mod navigation_blueprint;
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
    BuildingCatalog, BuildingCatalogError, BuildingCatalogRevision, BuildingDefinition,
    BuildingDefinitionId, BuildingRenderKey, BuildingVariantCreateInput,
    BuildingVariantCreateOutcome, create_building_variant, export_building_catalog_snapshot,
    merge_starter_extensions_into_catalog, replace_building_instance_definition,
    suggest_variant_definition_id, validate_building_definition_id,
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
};
pub use interior::{
    DoorAccessPolicy, DoorId, DoorRecord, DoorState, DoorStore, InteriorActivationOutcome,
    InteriorActivationOutcomeStore, InteriorActivationStatus, InteriorError,
    InteriorProfileCatalog, InteriorProfileId, NavigationReconcileOutcome,
    activate_building_interior, close_door, deactivate_building_interior, destroy_door, lock_door,
    open_door, portal_traversable, reconcile_all_building_navigation_runtimes,
    reconcile_building_navigation_runtime, refresh_building_navigation_runtime,
    space_route_for_unit, try_activate_interior_if_complete,
    try_open_door_at_portal_for_unit, try_open_door_for_unit,
};
pub use inventory::{
    BuildingInventoryContext, BuildingInventoryRemovalPolicy,
    building_container_access_policy, building_has_inventory, building_id_for_inventory,
    building_inventory_operational, can_unit_access_building_inventory, can_unit_access_inventory, set_building_container_locked,
    spill_position_for_building, unit_within_building_inventory_range,
    validate_building_inventory_links, validate_building_inventory_owner,
};
pub use inventory_binding::{
    BuildingInventoryBinding, BuildingInventoryBindingDefinition, BuildingInventoryBindingId,
    BuildingInventoryBindingSet, BuildingInventoryBindingStore,
    BuildingInventoryBindingValidationIssue, BuildingInventoryRole,
    building_inventory_bindings, default_building_inventory_binding,
    definition_requires_inventory_allocation, effective_inventory_binding_definitions,
    primary_building_inventory_id, resolve_building_inventory_binding,
    validate_building_catalog_inventory_bindings, validate_building_definition_inventory_bindings,
    validate_building_runtime_inventory_bindings, validate_operation_inventory_bindings,
    validate_selected_operation_inventory_bindings, validate_world_building_inventory_bindings,
};
pub use inventory_error::BuildingInventoryError;
#[cfg(feature = "dev")]
pub use navigation_blueprint::probe_segment_crosses_entrance_opening;
#[cfg(any(test, feature = "dev"))]
pub use navigation_blueprint::two_room_hut_navigation_blueprint;
pub use navigation_blueprint::{
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH,
    BlueprintAuthoritySource, BlueprintDiagnosticFocus, BlueprintDiagnosticLevel,
    BlueprintEditOutcome, BlueprintInspectionValidation, BlueprintPersistenceOutcome, BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision,
    BuildingNavigationBlueprintError, BuildingNavigationBlueprintId,
    BuildingNavigationBlueprintInstanceOverride,
    BuildingNavigationMovementAuthority, BuildingNavigationRuntime, BuildingNavigationRuntimeStore,
    DEFAULT_EXTERIOR_STAGING_OFFSET, ENTRANCE_CORNER_MARGIN, EntranceGenerationDiagnostics,
    GeometryGenerationDiagnostics, InteriorActivationCatalogs,
    NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH, NAVIGATION_BLUEPRINT_GENERATOR_VERSION, NavigationBlueprintCacheManifest, NavigationBlueprintGenerationStatus,
    NavigationEntranceDefinition, NavigationFloorDefinition, NavigationPolygon2d,
    NavigationRegionDefinition, NavigationVerticalTransitionDefinition,
    NavigationVerticalTransitionKind, ResolvedBuildingNavigationBlueprint, RuntimeNavigationFloor,
    RuntimeNavigationRegion, RuntimeTopologyFingerprint, add_entrance_on_floor,
    add_region_connection, add_region_on_floor, add_stair_transition, apply_blueprint_to_asset, blueprint_id_for_building, blueprint_topology_fingerprint, building_navigation_movement_authority,
    building_uses_blueprint_movement_authority, capture_building_navigation_topology_snapshot,
    classify_blueprint_authority, count_inheriting_instances, delete_entrance, delete_floor_vertex,
    delete_region, delete_region_connection, delete_transition,
    exterior_staging_for_entrance, format_region_deletion_error, insert_vertex_on_edge,
    interior_agent_fits_region, interior_navigation_move_target_at_position,
    interior_position_walkable, interior_segment_respects_region_boundary,
    load_building_navigation_blueprint_catalog, migrate_entrances_toward_boundaries,
    min_edge_clearance_meters, move_connection_from, move_connection_to, move_entrance,
    move_floor_vertex, move_transition_from, move_transition_to, movement_authority_label,
    nearest_boundary_projection as nearest_boundary_projection_entrance, point_in_polygon_xz,
    position_in_surface_entrance_portal, prepare_blueprint_for_save, region_interior_point,
    region_references,
    reposition_building_navigation_runtime, reset_instance_to_asset,
    resolve_building_navigation_blueprint, resolve_move_goal_space,
    resolve_navigation_space_at_position, resolve_navigation_start_space,
    resolve_surface_entrance_approach_position, resolve_surface_entrance_escape_position,
    runtime_topology_fingerprint, save_instance_blueprint, set_connection_radius,
    set_entrance_radius, set_entrance_region_key, set_transition_radius, surface_blueprint_support_blocks_position,
    surface_entrance_terrain_side_corridor_global_xz,
    surface_entrance_terrain_side_escape_global_xz, surface_position_in_entrance_access_corridor,
    surface_segment_respects_blueprint_boundaries,
    validate_blueprint_for_inspection,
};
#[cfg(feature = "data-import")]
pub use navigation_blueprint::{
    export_navigation_blueprint_catalog, hash_asset_path, import_navigation_blueprints_for_catalog, regenerate_navigation_blueprint_for_building,
    should_generate_navigation_blueprint,
};
pub use operation::{
    BuildingOperationParams, BuildingOperationPolicy,
    BuildingOperationSaveState, BuildingOperationState, BuildingOperationStore,
    BuildingProductionSaveState, BuildingProductionStore, BuildingWorkPriorityLevel, ControlSource,
    FarmProductionPhase, OperationDefinitionId, OperationLifecycle, PRODUCTION_PROGRESS_ONE_UNIT,
    PRODUCTION_STEPPING_MODEL, ProductionCommandError, ProductionProgress, ProductionValidationIssue, RepeatMode,
    apply_player_building_work_priority, apply_player_production_enabled,
    apply_player_production_selected_operation, assess_production_execution,
    building_work_priority_label, building_work_priority_level,
    building_work_priority_to_task_priority, building_work_priority_u8,
    building_work_priority_u8_for_level, cycle_production_selected_operation,
    execute_production_cycle, farm_growth_percent,
    farm_harvest_percent, farm_needs_harvest_worker,
    is_prispod_farm_definition, progress_to_percent,
    reconcile_farm_harvest_phase, reset_production_progress,
    set_building_work_priority, set_production_enabled, set_production_execution_mode,
    set_production_paused, set_production_repeat_count, set_production_selected_operation,
    step_all_farm_passive_growth, step_workstation_operation,
    validate_production_runtime, validate_production_runtime_with_catalogs,
    workstation_workers_for_building,
};
pub use operational_efficiency::{
    OperationalEfficiencyContext, OperationalEfficiencyError, OperationalEfficiencyReport,
    OperationalLimitingFactor, building_operational_efficiency, combine_output_efficiency,
};
pub use ownership::BuildingOwnership;
pub use placement::BuildingPlacement;
pub use placement_plan::{
    BuildingPlacementPlan, PLACEMENT_QUANTIZE_METERS, anchor_from_terrain_position,
    build_building_placement_plan, building_anchor_render_transform,
    building_placement_render_y,
    resolve_authoritative_building_placement,
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
pub use storage_policy::{
    apply_player_storage_accept_all,
    apply_player_storage_category_accepted, apply_player_storage_clear_all,
    building_is_storage_capable, building_storage_accepts_item,
    building_storage_accepts_item_for_definition, building_storage_policy, effective_storage_category_accepted,
    mark_settlement_storage_logistics_wakeup, mark_storage_logistics_dirty_for_inventory,
    storage_item_is_misfiled,
};
pub use store::ChunkBuildingStore;
pub use terrain_placement::{
    FOUNDATION_SLOPE_DEGREES, FOUNDATION_TEXTURE_TILE_METERS,
    FOUNDATION_TERRAIN_PENETRATION_FUDGE_METERS,
    FoundationPerimeterVertex, FoundationSkirtSpec, MAX_FOUNDATION_VISIBLE_DEPTH_METERS,
    PRESENTATION_TERRAIN_CLEARANCE_METERS, ResolvedBuildingPlacement, TerrainPlacementMode,
    derive_foundation_skirt_for_placement, footprint_horizontal_span_meters,
    foundation_slope_run_per_meter_drop, presentation_foundation_depth_meters,
    presentation_plane_normal, resolve_building_placement,
    sample_terrain_under_footprint, slope_degrees_from_plane_coefficients, terrain_clearance_sim,
};
pub use terrain_assessment::{
    AssessmentRebuildOutcome, AssessmentRebuildReport, BuildingFieldRequirementAssessment, BuildingTerrainAssessment, BuildingTerrainAssessmentKey,
    BuildingTerrainAssessmentStore, BuildingTerrainWarning,
    TerrainAssessmentCatalogs, TerrainAssessmentError, assess_building_terrain,
    assess_building_terrain_at_placement, evaluate_field_requirement,
    evaluate_field_requirement_assessment, format_coverage_display, format_efficiency_display,
    format_field_average_display, format_field_requirement_diagnostic, hash_sample_cells, primary_failure_for_assessment,
    rebuild_all_building_terrain_assessments, rebuild_building_terrain_assessment,
    resolve_building_field_sample_cells,
};
pub use transform_edit::{
    BuildingTransformCandidate, BuildingTransformCatalogs, BuildingTransformEditError,
    BuildingTransformEditOptions, BuildingTransformEditReport, update_building_transform,
};
pub use vitals::BuildingVitals;
