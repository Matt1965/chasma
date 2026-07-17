//! Terrain field authority: definitions, storage, and deterministic queries (ADR-101).

mod asset;
mod bake;
mod basis_points;
mod bootstrap;
mod catalog;
mod category;
mod compose;
mod contract;
mod definition;
mod diff;
mod error;
mod fixtures;
mod generate;
mod id;
mod import;
mod interpolate;
mod layer;
mod load;
mod load_tests;
mod mapping;
mod modifier;
mod overlay;
mod query;
mod region;
mod sample;
mod semantics;
mod source;
mod source_error;
mod store;
#[cfg(test)]
mod stress_tests;
#[cfg(test)]
mod tf2_tests;
mod tile;

pub use asset::{
    TERRAIN_FIELD_MANIFEST_VERSION, TERRAIN_FIELD_TILE_VERSION, TerrainFieldManifest,
    TerrainFieldManifestConfig, TerrainFieldManifestEntry, TerrainFieldTileFile, decode_manifest,
    decode_tile, tile_path_for_chunk,
};
pub use bake::{
    BiomeDependencyRef, BuildDependencies, FieldBuildReport, PackageReport, TerrainFieldStatistics,
    build_and_package_all_enabled, build_and_package_field, build_field_layer_from_profile,
    package_field_layers,
};
pub use basis_points::{BASIS_POINTS_ONE_HUNDRED_PERCENT, BasisPoints, BasisPointsError};
pub use bootstrap::{
    TerrainFieldBootstrapOutcome, bootstrap_terrain_fields_on_startup, bootstrap_with_extent,
    bootstrap_world_terrain_fields, reload_terrain_fields_with_invalidation,
};
pub use catalog::{TerrainFieldCatalog, TerrainFieldCatalogRon, validate_terrain_field_id};
pub use category::TerrainFieldCategory;
pub use compose::compose_terrain_field_value;
pub use contract::{
    TERRAIN_FIELD_BYTES_PER_TILE, TERRAIN_FIELD_INTERVALS_PER_CHUNK,
    TERRAIN_FIELD_SAMPLE_SPACING_METERS, TERRAIN_FIELD_SAMPLES_PER_EDGE,
    TERRAIN_FIELD_SAMPLES_PER_TILE, TerrainFieldContractError, expected_samples_per_edge,
    validate_world_config_for_fields,
};
pub use definition::TerrainFieldDefinition;
pub use diff::{TerrainFieldPackageDiff, diff_terrain_field_stores};
pub use error::{
    FieldAvailabilityReason, SharedEdgeAxis, TerrainFieldCatalogError, TerrainFieldDefinitionError,
    TerrainFieldLoadError, TerrainFieldQueryError, TerrainFieldStorageError,
};
pub use fixtures::{
    bootstrap_constant_field, bootstrap_dev_synthetic_fields, bootstrap_diagonal_gradient_field,
    bootstrap_x_gradient_field, bootstrap_z_gradient_field,
};
pub use generate::{
    GenerationContext, HeightfieldDependency, compose_field_seed, generate_field_value,
};
pub use id::{TerrainFieldId, TerrainFieldSourceProfileId};
pub use import::{
    DecodedFieldImage, TerrainFieldWorldRaster, decode_field_png_bytes, expand_u8_to_u16,
    partition_raster_to_tiles, raster_to_layer, resample_imported_image,
};
pub use interpolate::bilinear_sample_u16;
pub use layer::TerrainFieldLayer;
pub use load::{
    DEFAULT_TERRAIN_FIELD_MANIFEST_PATH, TERRAIN_FIELD_CATALOG_RON_PATH, TerrainFieldLoadSummary,
    load_terrain_field_catalog, load_terrain_fields_from_manifest, terrain_field_tile_path,
    try_load_terrain_fields_from_manifest,
};
pub use mapping::{
    FieldLocalSampleCoord, FieldMappingError, field_local_to_debug, fraction_to_q8,
    world_position_to_field_local,
};
pub use modifier::{
    TerrainFieldModifierEntry, TerrainFieldModifierKind, TerrainFieldModifierStore,
};
pub use overlay::TerrainFieldOverlayStyle;
pub use query::{
    field_sample_region_from_cells, sample_terrain_field_area, sample_terrain_field_at,
};
pub use region::FieldSampleRegion;
pub use sample::{
    FieldAreaAvailability, FieldAvailability, FieldSampleSource, TerrainFieldAreaReport,
    TerrainFieldInterpolationDebug, TerrainFieldSample,
};
pub use semantics::FieldValueSemantics;
pub use source::{
    GeneratedTerrainFieldSource, ImportedTerrainFieldSource, TERRAIN_FIELD_GENERATOR_VERSION,
    TERRAIN_FIELD_SOURCE_PROFILES_RON_PATH, TerrainFieldGeneratorDependency,
    TerrainFieldGeneratorKind, TerrainFieldImageChannel, TerrainFieldImageOrientation,
    TerrainFieldResampling, TerrainFieldSourceKind, TerrainFieldSourceProfileCatalog,
    TerrainFieldSourceProfileCatalogRon, TerrainFieldSourceProfileDefinition,
    TerrainFieldSourceProvenance, TerrainFieldValueRemap, TerrainFieldWorldBounds,
    load_terrain_field_source_profile_catalog, target_sample_dimensions,
};
pub use source_error::TerrainFieldSourceError;
pub use store::TerrainFieldStore;
pub use tile::TerrainFieldTile;

#[cfg(any(test, feature = "dev"))]
pub use catalog::starter::starter_definitions;
#[cfg(any(test, feature = "dev"))]
pub use source::starter::starter_source_profiles;
