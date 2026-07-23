//! Building navigation blueprints — gameplay interior navigation metadata (NV1.1+).
//!
//! Blueprints describe how units move through building interiors in building-local
//! space. They are independent from render meshes and collision geometry.

mod adapt;
mod cache;
mod catalog;
mod definition;
mod edit;
mod error;
mod id;
mod persistence;
mod report;
mod resolve;
mod runtime;
mod runtime_nav_tests;
mod source;
mod starter;
mod validate_inspection;

#[cfg(feature = "data-import")]
mod generate;
#[cfg(feature = "data-import")]
mod mesh;
#[cfg(feature = "data-import")]
mod pipeline;

pub use adapt::{
    BlueprintPortalTemplate, BlueprintSpaceTemplate, blueprint_portal_templates,
    blueprint_space_templates,
};
pub use cache::{
    NavigationBlueprintCacheEntry, NavigationBlueprintCacheManifest,
    NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH, NAVIGATION_BLUEPRINT_GENERATOR_VERSION,
};
pub use catalog::{
    BuildingNavigationBlueprintCatalog, BuildingNavigationBlueprintCatalogRevision,
    BuildingNavigationBlueprintCatalogRon, BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH,
    load_building_navigation_blueprint_catalog,
};
pub use definition::{
    BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintInstanceOverride, BuildingNavigationBlueprintMetadata,
    NavigationEntranceDefinition, NavigationFloorDefinition, NavigationPolygon2d,
    NavigationVerticalTransitionDefinition, NavigationVerticalTransitionKind,
};
pub use edit::{
    BlueprintEditOutcome, add_entrance_on_floor, add_stair_transition, delete_entrance,
    delete_floor_vertex, delete_transition, insert_vertex_on_edge, move_entrance,
    move_floor_vertex, move_transition_from, move_transition_to, prepare_blueprint_for_save,
    set_entrance_radius, set_transition_radius,
};
pub use error::BuildingNavigationBlueprintError;
pub use id::{BuildingNavigationBlueprintId, validate_navigation_blueprint_id};
pub use report::{
    NavigationBlueprintGenerationReport, NavigationBlueprintGenerationStatus,
    export_generation_reports_markdown,
};
pub use persistence::{
    BlueprintPersistenceOutcome, apply_blueprint_to_asset, count_inheriting_instances,
    reset_instance_to_asset, save_instance_blueprint,
};
pub use resolve::{
    ResolvedBuildingNavigationBlueprint, resolve_building_navigation_blueprint,
};
pub use source::{BlueprintAuthoritySource, classify_blueprint_authority};
pub use runtime::{
    BuildingNavigationRuntime, BuildingNavigationRuntimeStore, RuntimeNavigationFloor,
    build_navigation_runtime, interior_position_walkable, point_in_polygon_xz,
    position_in_surface_entrance_portal, register_building_navigation_profile,
    reposition_building_navigation_runtime, resolve_navigation_space_at_position,
    resolve_navigation_start_space,
};
pub use starter::{
    barn_navigation_blueprint, starter_navigation_blueprints, two_story_hut_navigation_blueprint,
};
pub use validate_inspection::{
    BlueprintDiagnosticFocus, BlueprintDiagnosticLevel, BlueprintInspectionValidation,
    BlueprintValidationDiagnostic, validate_blueprint_for_inspection,
};

#[cfg(feature = "data-import")]
pub use generate::{
    NavigationBlueprintGenerateInput, NavigationBlueprintGenerateOutput,
    blueprint_id_for_building, failed_report, generate_navigation_blueprint, hash_asset_path,
    should_generate_navigation_blueprint,
};
#[cfg(feature = "data-import")]
pub use mesh::{BuildingMeshAnalysisInput, LocalTriangle3d, PortalMarker3d, load_building_mesh_for_navigation};
#[cfg(feature = "data-import")]
pub use pipeline::{
    NAVIGATION_BLUEPRINT_REPORT_PATH, export_navigation_blueprint_catalog,
    import_navigation_blueprints_for_catalog, regenerate_navigation_blueprint_for_building,
};
