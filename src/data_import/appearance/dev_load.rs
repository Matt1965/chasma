//! Dev-only appearance profile catalog resolution from Excel import.

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::AppearanceProfileCatalog;

use super::import_appearance_profiles_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub fn resolve_dev_appearance_profile_catalog() -> AppearanceProfileCatalog {
    let path = dev_design_workbook_path();
    match try_import(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Appearance profile Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Appearance profile import warning: {warning}"),
                );
            }
            catalog
        }
        Err(err) => {
            let message = format!(
                "Appearance profile Excel import failed for {}: {err}\n\
                 Fix `Chasma Design.xlsx` (Appearance Profiles sheets) and restart.",
                path.display()
            );
            append_log_line(DEV_STARTUP_LOG_PATH, SESSION_HEADER, &message);
            panic!("{message}");
        }
    }
}

fn try_import(
    path: &Path,
) -> Result<(AppearanceProfileCatalog, crate::data_import::ImportSummary), DataImportError> {
    import_appearance_profiles_from_excel(path)
}
