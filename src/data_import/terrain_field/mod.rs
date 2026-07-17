#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;
mod validate;

#[cfg(feature = "data-import")]
pub use dev_load::{DEV_TERRAIN_FIELD_CATALOG_RON_PATH, resolve_dev_terrain_field_catalog};
#[cfg(feature = "data-import")]
pub use excel::{
    TERRAIN_FIELDS_SHEET_NAME, import_terrain_field_catalog_from_excel,
    import_terrain_fields_from_excel,
};
pub use schema::{
    OPTIONAL_COLUMNS as TERRAIN_FIELD_OPTIONAL_COLUMNS,
    REQUIRED_COLUMNS as TERRAIN_FIELD_REQUIRED_COLUMNS, TerrainFieldImportRow, parse_color_cell,
};
