pub mod bounds;
pub mod catalog;
pub mod generator_config;
pub mod import_config;
pub mod profile;
pub mod provenance;
pub mod remap;
pub mod starter;

pub use bounds::{TerrainFieldWorldBounds, target_sample_dimensions};
pub use catalog::{
    TERRAIN_FIELD_SOURCE_PROFILES_RON_PATH, TerrainFieldSourceProfileCatalog,
    TerrainFieldSourceProfileCatalogRon, load_terrain_field_source_profile_catalog,
};
pub use generator_config::{
    GeneratedTerrainFieldSource, TERRAIN_FIELD_GENERATOR_VERSION, TerrainFieldGeneratorDependency,
    TerrainFieldGeneratorKind,
};
pub use import_config::{
    ImportedTerrainFieldSource, TerrainFieldImageChannel, TerrainFieldImageOrientation,
    TerrainFieldOutsideCoveragePolicy, TerrainFieldResampling,
};
pub use profile::{TerrainFieldSourceKind, TerrainFieldSourceProfileDefinition};
pub use provenance::{TerrainFieldSourceProvenance, generator_kind_label};
pub use remap::TerrainFieldValueRemap;

#[cfg(any(test, feature = "dev"))]
mod export_test;
