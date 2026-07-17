pub mod registry;
pub mod starter;

#[cfg(all(test, feature = "data-import"))]
mod export_test;

pub use registry::{TerrainFieldCatalog, TerrainFieldCatalogRon, validate_terrain_field_id};
