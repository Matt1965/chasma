//! Dev-only animation profile catalog resolution (A1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::{AnimationProfileCatalog, starter_animation_profile_definitions};

use super::import_animation_profiles_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub fn resolve_dev_animation_profile_catalog() -> AnimationProfileCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_animation_profile_catalog(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Animation profile Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Animation profile import warning: {warning}"),
                );
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Animation profile Excel import failed for {} ({err}); using starter profiles",
                    path.display()
                ),
            );
            AnimationProfileCatalog::from_definitions(starter_animation_profile_definitions())
                .expect("starter animation profiles are valid")
        }
    }
}

fn try_import_dev_animation_profile_catalog(
    path: &Path,
) -> Result<(AnimationProfileCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_animation_profiles_from_excel(path)?;
    let catalog = AnimationProfileCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("animation profile catalog build failed: {err:?}"))
    })?;
    Ok((catalog, summary))
}
