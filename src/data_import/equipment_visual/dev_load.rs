//! Dev-only equipment visual catalog resolution from Excel import.

use std::path::Path;

use crate::data_import::DataImportError;
use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::EquipmentVisualCatalog;

use super::import_equipment_visuals_from_excel;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub fn resolve_dev_equipment_visual_catalog(
    items: &crate::world::ItemCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
) -> EquipmentVisualCatalog {
    let path = dev_design_workbook_path();
    match try_import(&path, items, appearance_profiles) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Equipment visual Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Equipment visual import warning: {warning}"),
                );
            }
            catalog
        }
        Err(err) => {
            let message = format!(
                "Equipment visual Excel import failed for {}: {err}\n\
                 Fix `Chasma Design.xlsx` (Equipment Visuals sheet) and restart.",
                path.display()
            );
            append_log_line(DEV_STARTUP_LOG_PATH, SESSION_HEADER, &message);
            panic!("{message}");
        }
    }
}

fn try_import(
    path: &Path,
    items: &crate::world::ItemCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
) -> Result<(EquipmentVisualCatalog, crate::data_import::ImportSummary), DataImportError> {
    import_equipment_visuals_from_excel(path, items, appearance_profiles)
}
