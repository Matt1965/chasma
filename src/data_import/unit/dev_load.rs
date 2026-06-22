//! Dev-only unit catalog resolution from Excel import (ADR-027 U1).

use std::path::Path;

use bevy::prelude::*;

use crate::world::{starter_unit_definitions, UnitCatalog};

use super::import_units_from_excel;
use crate::data_import::{DataImportError, ImportSummary};

/// Default Excel workbook path for dev unit authoring (`Chasma Design` pipeline).
pub const DEV_UNIT_EXCEL_PATH: &str = "Chasma Design.xlsx";

/// Load [`UnitCatalog`] for dev startup: Excel import with starter fallback.
pub fn resolve_dev_unit_catalog() -> UnitCatalog {
    match try_import_dev_unit_catalog(Path::new(DEV_UNIT_EXCEL_PATH)) {
        Ok((catalog, summary)) => {
            info!(
                "Unit Excel import: processed={} valid={} failed={} warnings={}",
                summary.rows_processed,
                summary.rows_valid,
                summary.rows_failed,
                summary.warnings.len(),
            );
            for warning in &summary.warnings {
                warn!("Unit import warning: {warning}");
            }
            catalog
        }
        Err(err) => {
            warn!(
                "Unit Excel import failed ({err}); using starter catalog ({} definitions)",
                starter_unit_definitions().len()
            );
            UnitCatalog::default()
        }
    }
}

fn try_import_dev_unit_catalog(
    path: &Path,
) -> Result<(UnitCatalog, ImportSummary), DataImportError> {
    let (definitions, summary) = import_units_from_excel(path)?;
    let catalog = UnitCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("unit catalog build failed: {err:?}"))
    })?;
    Ok((catalog, summary))
}
