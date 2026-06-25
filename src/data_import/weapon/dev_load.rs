//! Dev-only weapon catalog resolution from Excel import (ADR-054 C1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{append_log_line, DEV_STARTUP_LOG_PATH};
use crate::world::WeaponCatalog;

use super::import_weapons_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

/// Load [`WeaponCatalog`] for dev startup from the design workbook `Weapons` sheet.
pub fn resolve_dev_weapon_catalog() -> WeaponCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_weapon_catalog(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Weapon Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Weapon import warning: {warning}"),
                );
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Weapon Excel import failed for {} ({err}); using starter WeaponCatalog",
                    path.display()
                ),
            );
            WeaponCatalog::default()
        }
    }
}

fn try_import_dev_weapon_catalog(
    path: &Path,
) -> Result<(WeaponCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_weapons_from_excel(path)?;
    let catalog = WeaponCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("weapon catalog build failed: {err}"))
    })?;
    Ok((catalog, summary))
}
