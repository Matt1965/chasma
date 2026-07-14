//! Dev-only inventory profile catalog resolution (ADR-087 I1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::InventoryProfileCatalog;

use super::import_inventory_profiles_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub const DEV_INVENTORY_PROFILE_CATALOG_RON_PATH: &str = "assets/inventory/profiles.ron";

pub fn resolve_dev_inventory_profile_catalog() -> InventoryProfileCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_inventory_profiles(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Inventory profile Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Inventory profile import warning: {warning}"),
                );
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Inventory profile Excel import failed for {} ({err}); using starter profiles",
                    path.display()
                ),
            );
            InventoryProfileCatalog::default()
        }
    }
}

fn try_import_dev_inventory_profiles(
    path: &Path,
) -> Result<(InventoryProfileCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_inventory_profiles_from_excel(path)?;
    let catalog = InventoryProfileCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("inventory profile catalog build failed: {err}"))
    })?;
    if let Err(err) = crate::data_import::ron::export_inventory_profiles_to_ron(
        Path::new(DEV_INVENTORY_PROFILE_CATALOG_RON_PATH),
        catalog.definitions(),
    ) {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Inventory profile RON export failed: {err}"),
        );
    }
    Ok((catalog, summary))
}
