//! Dev-only unit catalog resolution from Excel import (ADR-027 U1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::UnitCatalog;

use super::import_units_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

/// Load [`UnitCatalog`] for dev startup from the design workbook `Units` sheet.
pub fn resolve_dev_unit_catalog(weapons: &crate::world::WeaponCatalog) -> UnitCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_unit_catalog(&path, weapons) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Unit Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Unit import warning: {warning}"),
                );
            }
            let ids: Vec<_> = catalog
                .definitions()
                .iter()
                .map(|def| def.id.as_str())
                .collect();
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!("Unit catalog ids: {}", ids.join(", ")),
            );
            let renderable: Vec<_> = catalog
                .definitions()
                .iter()
                .filter(|def| def.render_key.0.is_some())
                .map(|def| def.id.as_str())
                .collect();
            if renderable.is_empty() {
                append_log_line(
                    DEV_STARTUP_LOG_PATH,
                    SESSION_HEADER,
                    "Unit catalog has no renderable definitions (add a `File Path` column to the \
                     Units sheet, e.g. `\\units\\robot.glb` on the robot row)",
                );
            } else {
                append_log_line(
                    DEV_STARTUP_LOG_PATH,
                    SESSION_HEADER,
                    &format!("Unit catalog render keys: {}", renderable.join(", ")),
                );
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Unit Excel import failed for {} ({err}); dev unit catalog is empty",
                    path.display()
                ),
            );
            UnitCatalog::from_definitions(Vec::new()).expect("empty unit catalog is valid")
        }
    }
}

fn try_import_dev_unit_catalog(
    path: &Path,
    weapons: &crate::world::WeaponCatalog,
) -> Result<(UnitCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_units_from_excel(path, weapons)?;
    let catalog = UnitCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("unit catalog build failed: {err:?}"))
    })?;
    Ok((catalog, summary))
}
