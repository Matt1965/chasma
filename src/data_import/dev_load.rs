//! Dev-only doodad catalog resolution from Excel import (R6).

use std::path::Path;

use bevy::prelude::*;

use crate::world::{DoodadCatalog, starter_definitions};

use super::{export_doodads_to_ron, import_doodads_from_excel, DataImportError, ImportSummary};

/// Default Excel workbook path for dev doodad authoring (`Chasma Design` pipeline).
pub const DEV_DOODAD_EXCEL_PATH: &str = "source_data/doodads/Doodads.xlsx";

/// Optional RON export written after a successful dev import.
pub const DEV_DOODAD_CATALOG_RON_PATH: &str = "assets/doodads/catalog.ron";

/// Load [`DoodadCatalog`] for dev startup: Excel import with starter fallback.
pub fn resolve_dev_doodad_catalog() -> DoodadCatalog {
    match try_import_dev_doodad_catalog(Path::new(DEV_DOODAD_EXCEL_PATH)) {
        Ok((catalog, summary)) => {
            info!(
                "Doodad Excel import: processed={} valid={} failed={} warnings={}",
                summary.rows_processed,
                summary.rows_valid,
                summary.rows_failed,
                summary.warnings.len(),
            );
            for warning in &summary.warnings {
                warn!("Doodad import warning: {warning}");
            }
            catalog
        }
        Err(err) => {
            warn!(
                "Doodad Excel import failed ({err}); using starter catalog ({} definitions)",
                starter_definitions().len()
            );
            DoodadCatalog::default()
        }
    }
}

fn try_import_dev_doodad_catalog(
    path: &Path,
) -> Result<(DoodadCatalog, ImportSummary), DataImportError> {
    let (definitions, summary) = import_doodads_from_excel(path)?;
    if let Err(err) = export_doodads_to_ron(Path::new(DEV_DOODAD_CATALOG_RON_PATH), &definitions)
    {
        warn!("Doodad RON export failed: {err}");
    }
    let catalog = DoodadCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("catalog build failed: {err:?}"))
    })?;
    Ok((catalog, summary))
}
