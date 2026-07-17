//! Dev-only terrain field catalog resolution (ADR-101 TF1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::TerrainFieldCatalog;

use super::excel::import_terrain_field_catalog_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub const DEV_TERRAIN_FIELD_CATALOG_RON_PATH: &str = "assets/terrain_fields/catalog.ron";

pub fn resolve_dev_terrain_field_catalog() -> TerrainFieldCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_terrain_fields(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Terrain field Excel import ({}): processed={} valid={} failed={} warnings={}",
                    path.display(),
                    summary.rows_processed,
                    summary.rows_valid,
                    summary.rows_failed,
                    summary.warnings.len(),
                ),
            );
            for warning in &summary.warnings {
                append_log_line(
                    DEV_STARTUP_LOG_PATH,
                    SESSION_HEADER,
                    &format!("Terrain field import warning: {warning}"),
                );
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Terrain field Excel import failed for {} ({err}); using starter terrain field catalog",
                    path.display()
                ),
            );
            TerrainFieldCatalog::default()
        }
    }
}

fn try_import_dev_terrain_fields(
    path: &Path,
) -> Result<(TerrainFieldCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (catalog, summary) = import_terrain_field_catalog_from_excel(path)?;
    if let Err(err) = crate::data_import::ron::export_terrain_fields_to_ron(
        Path::new(DEV_TERRAIN_FIELD_CATALOG_RON_PATH),
        catalog.definitions(),
    ) {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Terrain field RON export failed: {err}"),
        );
    }
    Ok((catalog, summary))
}
