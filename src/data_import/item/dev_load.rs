//! Dev-only item catalog resolution from Excel import (ADR-087 I1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::{ItemCatalog, ItemCategoryCatalog};

use super::import_item_catalog_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub const DEV_ITEM_CATALOG_RON_PATH: &str = "assets/items/catalog.ron";

pub fn resolve_dev_item_catalog() -> (ItemCategoryCatalog, ItemCatalog) {
    let path = dev_design_workbook_path();
    match try_import_dev_item_catalog(&path) {
        Ok((categories, items, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Item Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Item import warning: {warning}"),
                );
            }
            (categories, items)
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Item Excel import failed for {} ({err}); using starter item catalogs",
                    path.display()
                ),
            );
            (ItemCategoryCatalog::default(), ItemCatalog::default())
        }
    }
}

fn try_import_dev_item_catalog(
    path: &Path,
) -> Result<
    (
        ItemCategoryCatalog,
        ItemCatalog,
        crate::data_import::ImportSummary,
    ),
    DataImportError,
> {
    let (categories, items, summary) = import_item_catalog_from_excel(path)?;
    if let Err(err) = crate::data_import::ron::export_items_to_ron(
        Path::new(DEV_ITEM_CATALOG_RON_PATH),
        categories.definitions(),
        items.definitions(),
    ) {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Item RON export failed: {err}"),
        );
    }
    Ok((categories, items, summary))
}
