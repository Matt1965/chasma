//! Dev-only building catalog resolution from Excel import (B1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::{BuildingCatalog, BuildingCategoryCatalog};

use super::import_building_catalog_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

pub const DEV_BUILDING_CATALOG_RON_PATH: &str = "assets/buildings/catalog.ron";

/// Load building categories and definitions for dev startup from the design workbook.
pub fn resolve_dev_building_catalog(
    inventory_profiles: &crate::world::InventoryProfileCatalog,
) -> (BuildingCategoryCatalog, BuildingCatalog) {
    let path = dev_design_workbook_path();
    match try_import_dev_building_catalog(&path, inventory_profiles) {
        Ok((categories, buildings, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Building Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Building import warning: {warning}"),
                );
            }
            (categories, buildings)
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Building Excel import failed for {} ({err}); using starter building catalogs",
                    path.display()
                ),
            );
            (
                BuildingCategoryCatalog::default(),
                BuildingCatalog::default(),
            )
        }
    }
}

fn try_import_dev_building_catalog(
    path: &Path,
    inventory_profiles: &crate::world::InventoryProfileCatalog,
) -> Result<
    (
        BuildingCategoryCatalog,
        BuildingCatalog,
        crate::data_import::ImportSummary,
    ),
    DataImportError,
> {
    let (categories, buildings, summary) =
        import_building_catalog_from_excel(path, inventory_profiles)?;
    if let Err(err) = super::super::ron::export_buildings_to_ron(
        Path::new(DEV_BUILDING_CATALOG_RON_PATH),
        categories.definitions(),
        buildings.definitions(),
    ) {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Building RON export failed: {err}"),
        );
    }
    Ok((categories, buildings, summary))
}
