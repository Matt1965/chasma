//! Building navigation blueprints — gameplay interior navigation metadata (NV1.1+).
//!
//! Blueprints describe how units move through building interiors in building-local
//! space. They are independent from render meshes and collision geometry.

mod adapt;
pub mod authority;
mod cache;
mod catalog;
mod definition;
mod edit;
mod entrance_geometry;
mod error;
mod fixtures;
mod id;
mod interior_entry_tests;
mod migrate;
mod multi_region_nav_tests;
#[cfg(test)]
mod opening_aperture_tests;
mod opening_clearance_tests;
mod opening_geometry;
#[cfg(test)]
mod opening_match_tests;
mod persistence;
#[cfg(test)]
mod real_hut_activation_tests;
mod report;
mod resolve;
mod runtime;
#[cfg(feature = "dev")]
pub use runtime::probe_segment_crosses_entrance_opening;
#[cfg(test)]
mod door_binding_tests;
mod runtime_nav_tests;
mod source;
mod starter;
#[cfg(test)]
mod surface_entry_diagnostics;
#[cfg(test)]
mod surface_entry_movement_tests;
#[cfg(test)]
mod surface_entry_tests;
#[cfg(test)]
mod surface_exit_movement_tests;
#[cfg(test)]
mod surface_exit_tests;
mod surface_support;
#[cfg(test)]
mod surface_support_tests;
mod validate_inspection;

#[cfg(feature = "data-import")]
mod generate;
#[cfg(feature = "data-import")]
mod mesh;
#[cfg(feature = "data-import")]
mod pipeline;
#[cfg(feature = "data-import")]
mod region_extract;

pub use adapt::{
    BlueprintPortalTemplate, BlueprintSpaceTemplate, blueprint_portal_templates,
    blueprint_space_templates, floor_key_from_region_space_key, region_space_key,
};
pub use authority::{
    BuildingNavigationMovementAuthority, building_navigation_movement_authority,
    building_uses_blueprint_movement_authority, movement_authority_label,
};
pub use cache::{
    NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH, NAVIGATION_BLUEPRINT_GENERATOR_VERSION,
    NavigationBlueprintCacheEntry, NavigationBlueprintCacheManifest,
};
pub use catalog::{
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision, BuildingNavigationBlueprintCatalogRon,
    load_building_navigation_blueprint_catalog,
};
pub use definition::{
    BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintInstanceOverride, BuildingNavigationBlueprintMetadata,
    MIN_CONNECTION_RADIUS, MIN_REGION_AREA, NavigationEntranceDefinition,
    NavigationFloorDefinition, NavigationPolygon2d, NavigationRegionConnectionDefinition,
    NavigationRegionConnectionKind, NavigationRegionDefinition,
    NavigationVerticalTransitionDefinition, NavigationVerticalTransitionKind, point_inside_polygon,
    single_region_floor,
};
pub use edit::{
    BlueprintEditOutcome, RegionReference, add_entrance_on_floor, add_region_connection,
    add_region_on_floor, add_stair_transition, delete_entrance, delete_floor_vertex, delete_region,
    delete_region_connection, delete_transition, format_region_deletion_error,
    insert_vertex_on_edge, move_connection_from, move_connection_to, move_entrance,
    move_floor_vertex, move_transition_from, move_transition_to, prepare_blueprint_for_save,
    region_interior_point, region_references, set_connection_bidirectional,
    set_connection_door_key, set_connection_enabled, set_connection_kind, set_connection_radius,
    set_entrance_radius, set_entrance_region_key, set_region_display_label, set_region_room_tag,
    set_transition_radius,
};
pub use entrance_geometry::{
    BoundaryProjection, DEFAULT_EXTERIOR_STAGING_OFFSET, DEFAULT_INTERIOR_LANDING_OFFSET,
    ENTRANCE_BOUNDARY_TOLERANCE, ENTRANCE_CORNER_MARGIN, ENTRANCE_EDGE_SNAP_MAX_DISTANCE,
    ENTRANCE_MIGRATION_SNAP_TOLERANCE, EntranceReanchorOutcome, apply_threshold_geometry,
    derive_exterior_staging_xz, exterior_staging_for_entrance, migrate_entrances_toward_boundaries,
    nearest_boundary_projection, point_on_boundary_within_tolerance,
    reanchor_entrance_to_region_boundary, reanchor_entrances_after_region_edit,
};
pub use error::BuildingNavigationBlueprintError;
pub use fixtures::{
    corridor_hut_navigation_blueprint, dual_doorway_navigation_blueprint,
    one_region_doorless_navigation_blueprint, two_floor_two_room_navigation_blueprint,
    two_room_hut_navigation_blueprint,
};
pub use id::{
    BuildingNavigationBlueprintId, blueprint_id_for_building, validate_navigation_blueprint_id,
};
pub use migrate::{BlueprintMigrationReport, migrate_blueprint_to_current};
pub use persistence::{
    BlueprintPersistenceOutcome, BlueprintPropagationCounts, InteriorActivationCatalogs,
    apply_blueprint_to_asset, count_inheriting_instances, reset_instance_to_asset,
    save_instance_blueprint,
};
#[cfg(feature = "data-import")]
pub use region_extract::RegionGeneratorConfig;
pub use report::{
    EntranceGenerationDiagnostics, GeometryGenerationDiagnostics,
    NavigationBlueprintGenerationReport, NavigationBlueprintGenerationStatus,
    export_generation_reports_markdown,
};
pub use resolve::{ResolvedBuildingNavigationBlueprint, resolve_building_navigation_blueprint};
pub use runtime::{
    BuildingNavigationRuntime, BuildingNavigationRuntimeStore, BuildingNavigationTopologySnapshot,
    RuntimeNavigationFloor, RuntimeNavigationRegion, RuntimeTopologyFingerprint,
    blueprint_region_count, blueprint_topology_fingerprint, build_navigation_runtime,
    capture_building_navigation_topology_snapshot, interior_agent_fits_region,
    interior_navigation_move_target_at_position, interior_position_walkable,
    interior_segment_respects_region_boundary, min_edge_clearance_meters, point_in_polygon_xz,
    position_in_surface_entrance_portal, register_building_navigation_profile,
    reposition_building_navigation_runtime, resolve_move_goal_space,
    resolve_navigation_space_at_position, resolve_navigation_start_space,
    runtime_topology_fingerprint, surface_segment_respects_blueprint_boundaries,
};
pub use source::{BlueprintAuthoritySource, classify_blueprint_authority};
pub use starter::{
    barn_navigation_blueprint, starter_navigation_blueprints, two_story_hut_navigation_blueprint,
};
pub use surface_support::{
    resolve_surface_entrance_approach_position, resolve_surface_entrance_escape_position,
    resolve_surface_entrance_terrain_side_corridor_position,
    surface_blueprint_support_blocks_position, surface_entrance_terrain_side_corridor_global_xz,
    surface_entrance_terrain_side_escape_global_xz, surface_position_in_entrance_access_corridor,
};
pub use validate_inspection::{
    BlueprintDiagnosticFocus, BlueprintDiagnosticLevel, BlueprintInspectionValidation,
    BlueprintValidationDiagnostic, validate_blueprint_for_inspection,
};

#[cfg(feature = "data-import")]
pub use generate::{
    NavigationBlueprintGenerateInput, NavigationBlueprintGenerateOutput, failed_report,
    generate_navigation_blueprint, hash_asset_path, logical_portal_group_key,
    navigation_blueprint_generation_rejection, navigation_mesh_source_display,
    navigation_mesh_source_label, should_generate_navigation_blueprint,
};
#[cfg(feature = "data-import")]
pub use mesh::{
    BuildingMeshAnalysisInput, LocalTriangle3d, PortalMarker3d, load_building_mesh_for_navigation,
    load_building_mesh_for_navigation_with_fallback,
};
#[cfg(feature = "data-import")]
pub use pipeline::{
    NAVIGATION_BLUEPRINT_REPORT_PATH, export_navigation_blueprint_catalog,
    generate_navigation_blueprint_draft_for_definition, import_navigation_blueprints_for_catalog,
    regenerate_navigation_blueprint_for_building,
};
