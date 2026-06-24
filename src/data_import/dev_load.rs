//! Dev-only doodad catalog resolution from Excel import (R6).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{append_log_line, DEV_STARTUP_LOG_PATH};
use crate::world::DoodadCatalog;

use super::{export_doodads_to_ron, import_doodads_from_excel, DataImportError};

const SESSION_HEADER: &str = "# chasma dev startup log";

/// Optional RON export written after a successful dev import.
pub const DEV_DOODAD_CATALOG_RON_PATH: &str = "assets/doodads/catalog.ron";

/// Load [`DoodadCatalog`] for dev startup from the design workbook `Doodads` sheet.
pub fn resolve_dev_doodad_catalog() -> DoodadCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_doodad_catalog(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Doodad Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Doodad import warning: {warning}"),
                );
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Doodad Excel import failed for {} ({err}); dev doodad catalog is empty",
                    path.display()
                ),
            );
            DoodadCatalog::default()
        }
    }
}

fn try_import_dev_doodad_catalog(
    path: &Path,
) -> Result<(DoodadCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_doodads_from_excel(path)?;
    if let Err(err) = export_doodads_to_ron(Path::new(DEV_DOODAD_CATALOG_RON_PATH), &definitions)
    {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Doodad RON export failed: {err}"),
        );
    }
    let catalog = DoodadCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("catalog build failed: {err:?}"))
    })?;
    Ok((catalog, summary))
}
