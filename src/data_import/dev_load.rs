//! Dev-only doodad catalog resolution from Excel import (R6).

use std::collections::HashMap;
use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::{DoodadCatalog, DoodadDefinitionId};

use super::{DataImportError, export_doodads_to_ron, import_doodads_from_excel};

const SESSION_HEADER: &str = "# chasma dev startup log";

/// Optional RON export written after a successful dev import.
pub const DEV_DOODAD_CATALOG_RON_PATH: &str = "assets/doodads/catalog.ron";

/// Legacy ids still referenced by scenes, procgen snapshots, and tests.
const DEV_DOODAD_LEGACY_RENDER_KEYS: &[(&str, &str)] = &[
    ("tree_oak", "tree/oak"),
    ("tree_dead", "tree/dead"),
    ("rock_small", "rock/small"),
    ("rock_large", "rock/large"),
    ("bush_scrub", "bush/scrub"),
    ("ruin_stone", "ruin/stone"),
    ("resource_node_iron", "resource/iron"),
    ("interior_chair", "interior/chair"),
];

fn dev_doodad_legacy_aliases(catalog: &DoodadCatalog) -> HashMap<DoodadDefinitionId, DoodadDefinitionId> {
    let mut aliases = HashMap::new();
    for (legacy_id, render_key) in DEV_DOODAD_LEGACY_RENDER_KEYS {
        let legacy = DoodadDefinitionId::new(*legacy_id);
        if catalog.get(&legacy).is_some() {
            continue;
        }
        let Some(canonical) = catalog.definitions().iter().find(|definition| {
            definition.render_key.as_str() == Some(*render_key)
        }) else {
            continue;
        };
        aliases.insert(legacy, canonical.id.clone());
    }
    aliases
}

/// Load [`DoodadCatalog`] for dev startup from the design workbook `Doodads` sheet.
pub fn resolve_dev_doodad_catalog(
    sizing_reports: Option<&mut Vec<crate::world::AssetSizingReport>>,
) -> DoodadCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_doodad_catalog(&path) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Doodad Excel import ({}): processed={} valid={} failed={} warnings={}",
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
                    &format!("Doodad import warning: {warning}"),
                );
            }
            if let Some(reports) = sizing_reports {
                reports.extend(summary.sizing_reports);
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Doodad Excel import failed for {} ({err}); dev doodad catalog is empty",
                    path.display()
                ),
            );
            DoodadCatalog::default()
        }
    }
}

fn try_import_dev_doodad_catalog(
    path: &Path,
) -> Result<(DoodadCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_doodads_from_excel(path)?;
    if let Err(err) = export_doodads_to_ron(Path::new(DEV_DOODAD_CATALOG_RON_PATH), &definitions) {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Doodad RON export failed: {err}"),
        );
    }
    let catalog = DoodadCatalog::from_definitions(definitions)
        .map_err(|err| DataImportError::WorkbookOpen(format!("catalog build failed: {err:?}")))?;
    let alias_map = dev_doodad_legacy_aliases(&catalog);
    let catalog = catalog
        .with_legacy_aliases(alias_map)
        .map_err(|err| DataImportError::WorkbookOpen(format!("catalog aliases failed: {err:?}")))?;
    Ok((catalog, summary))
}
