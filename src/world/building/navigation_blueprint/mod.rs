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
    blueprint_space_templates,
};
pub use authority::{
    BuildingNavigationMovementAuthority, building_navigation_movement_authority,
    building_uses_blueprint_movement_authority, movement_authority_label,
};
pub use cache::{
    NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH, NAVIGATION_BLUEPRINT_GENERATOR_VERSION, NavigationBlueprintCacheManifest,
};
pub use catalog::{
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision,
    load_building_navigation_blueprint_catalog,
};
pub use definition::{
    BuildingNavigationBlueprint,
    BuildingNavigationBlueprintInstanceOverride, NavigationEntranceDefinition,
    NavigationFloorDefinition, NavigationPolygon2d, NavigationRegionDefinition,
    NavigationVerticalTransitionDefinition, NavigationVerticalTransitionKind,
};
pub use edit::{
    BlueprintEditOutcome, add_entrance_on_floor, add_region_connection,
    add_region_on_floor, add_stair_transition, delete_entrance, delete_floor_vertex, delete_region,
    delete_region_connection, delete_transition, format_region_deletion_error,
    insert_vertex_on_edge, move_connection_from, move_connection_to, move_entrance,
    move_floor_vertex, move_transition_from, move_transition_to, prepare_blueprint_for_save,
    region_interior_point, region_references, set_connection_radius,
    set_entrance_radius, set_entrance_region_key,
    set_transition_radius,
};
pub use entrance_geometry::{
    DEFAULT_EXTERIOR_STAGING_OFFSET,
    ENTRANCE_BOUNDARY_TOLERANCE, ENTRANCE_CORNER_MARGIN, exterior_staging_for_entrance, migrate_entrances_toward_boundaries,
    nearest_boundary_projection,
};
pub use error::BuildingNavigationBlueprintError;
pub use fixtures::two_room_hut_navigation_blueprint;
pub use id::{
    BuildingNavigationBlueprintId, blueprint_id_for_building,
};
pub use persistence::{
    BlueprintPersistenceOutcome, InteriorActivationCatalogs,
    apply_blueprint_to_asset, count_inheriting_instances, reset_instance_to_asset,
    save_instance_blueprint,
};
pub use report::{
    EntranceGenerationDiagnostics, GeometryGenerationDiagnostics, NavigationBlueprintGenerationStatus,
};
pub use resolve::{ResolvedBuildingNavigationBlueprint, resolve_building_navigation_blueprint};
pub use runtime::{
    BuildingNavigationRuntime, BuildingNavigationRuntimeStore,
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
pub use surface_support::{
    resolve_surface_entrance_approach_position, resolve_surface_entrance_escape_position,
    surface_blueprint_support_blocks_position, surface_entrance_terrain_side_corridor_global_xz,
    surface_entrance_terrain_side_escape_global_xz, surface_position_in_entrance_access_corridor,
};
pub use validate_inspection::{
    BlueprintDiagnosticFocus, BlueprintDiagnosticLevel, BlueprintInspectionValidation, validate_blueprint_for_inspection,
};

#[cfg(feature = "data-import")]
pub use generate::{
    hash_asset_path, should_generate_navigation_blueprint,
};
#[cfg(feature = "data-import")]
pub use pipeline::{
    export_navigation_blueprint_catalog, import_navigation_blueprints_for_catalog,
    regenerate_navigation_blueprint_for_building,
};
