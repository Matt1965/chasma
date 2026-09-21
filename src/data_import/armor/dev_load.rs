use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::ArmorProfileCatalog;

use super::import_armor_profiles_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub fn resolve_dev_armor_profile_catalog() -> ArmorProfileCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_armor_profile_catalog(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Armor profile Excel import ({}): processed={} valid={} failed={} warnings={}",
                    path.display(),
                    summary.rows_processed,
                    summary.rows_valid,
                    summary.rows_failed,
                    summary.warnings.len(),
                ),
            );
            catalog
        }
        Err(err) => {
            let message = format!(
                "Armor profile Excel import failed for {}: {err}\n\
                 Fix `Chasma Design.xlsx` (Armor Profiles sheet) and restart.",
                path.display()
            );
            append_log_line(DEV_STARTUP_LOG_PATH, SESSION_HEADER, &message);
            panic!("{message}");
        }
    }
}

fn try_import_dev_armor_profile_catalog(
    path: &Path,
) -> Result<(ArmorProfileCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_armor_profiles_from_excel(path)?;
    let catalog = ArmorProfileCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("armor profile catalog build failed: {err}"))
    })?;
    Ok((catalog, summary))
}
