//! Dev-only relationship catalog resolution from Excel import (ADR-132).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::{AuthoredRelationshipCatalog, FactionCatalog, SpeciesCatalog};

use super::matrix_import::{
    RELATIONSHIP_MATRIX_REPORT_PATH, export_relationship_matrix_report_markdown,
};
use super::{
    import_authored_relationship_matrices_from_excel, import_faction_catalog_from_excel,
    import_species_catalog_from_excel,
};

const SESSION_HEADER: &str = "# chasma dev startup log";

pub fn resolve_dev_faction_catalog() -> FactionCatalog {
    let path = dev_design_workbook_path();
    match import_faction_catalog_from_excel(&path) {
        Ok((catalog, summary)) => {
            log_summary("Faction", &path, &summary);
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Faction Excel import failed for {} ({err}); using starter faction catalog",
                    path.display()
                ),
            );
            FactionCatalog::default()
        }
    }
}

pub fn resolve_dev_species_catalog() -> SpeciesCatalog {
    let path = dev_design_workbook_path();
    match import_species_catalog_from_excel(&path) {
        Ok((catalog, summary)) => {
            log_summary("Species", &path, &summary);
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Species Excel import failed for {} ({err}); using starter species catalog",
                    path.display()
                ),
            );
            SpeciesCatalog::default()
        }
    }
}

/// Load authored relationship matrices after identity catalogs are available.
///
/// On import failure the store remains **empty** — missing edges contribute 0 conceptually.
/// No fallback relationship values are invented.
pub fn resolve_dev_authored_relationship_catalog(
    factions: &FactionCatalog,
    species: &SpeciesCatalog,
) -> AuthoredRelationshipCatalog {
    let path = dev_design_workbook_path();
    match import_authored_relationship_matrices_from_excel(&path, factions, species) {
        Ok((catalog, summary)) => {
            log_matrix_summary(&path, &summary);
            write_matrix_report(&catalog, &summary);
            catalog
        }
        Err(err) => {
            let summary = crate::data_import::relationship::RelationshipMatrixImportSummary {
                errors: vec![err.to_string()],
                ..Default::default()
            };
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Relationship matrix Excel import failed for {} ({err}); authored relationship catalog is empty",
                    path.display()
                ),
            );
            write_matrix_report(&AuthoredRelationshipCatalog::default(), &summary);
            AuthoredRelationshipCatalog::default()
        }
    }
}

pub fn import_relationship_identity_catalogs_from_excel(
    path: &Path,
) -> Result<(FactionCatalog, SpeciesCatalog), crate::data_import::DataImportError> {
    let (factions, _) = import_faction_catalog_from_excel(path)?;
    let (species, _) = import_species_catalog_from_excel(path)?;
    Ok((factions, species))
}

fn log_summary(label: &str, path: &Path, summary: &crate::data_import::ImportSummary) {
    append_log_line(
        DEV_STARTUP_LOG_PATH,
        SESSION_HEADER,
        &format!(
            "{label} Excel import ({}): processed={} valid={} failed={} warnings={}",
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
            &format!("{label} import warning: {warning}"),
        );
    }
}

fn log_matrix_summary(
    path: &Path,
    summary: &crate::data_import::relationship::RelationshipMatrixImportSummary,
) {
    append_log_line(
        DEV_STARTUP_LOG_PATH,
        SESSION_HEADER,
        &format!(
            "Relationship matrix Excel import ({}): sheets={} imported={} aborted={} edges={} warnings={} errors={}",
            path.display(),
            summary.sheets_discovered,
            summary.sheets_imported,
            summary.sheets_aborted,
            summary.edges_imported,
            summary.warnings.len(),
            summary.errors.len(),
        ),
    );
    for warning in &summary.warnings {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Relationship matrix import warning: {warning}"),
        );
    }
    for error in &summary.errors {
        append_log_line(
            DEV_STARTUP_LOG_PATH,
            SESSION_HEADER,
            &format!("Relationship matrix import error: {error}"),
        );
    }
}

fn write_matrix_report(
    catalog: &AuthoredRelationshipCatalog,
    summary: &crate::data_import::relationship::RelationshipMatrixImportSummary,
) {
    match export_relationship_matrix_report_markdown(
        Path::new(RELATIONSHIP_MATRIX_REPORT_PATH),
        summary,
        catalog,
    ) {
        Ok(()) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Relationship matrix report: {} edges → {}",
                    catalog.len(),
                    RELATIONSHIP_MATRIX_REPORT_PATH
                ),
            );
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!("Relationship matrix report export failed: {err}"),
            );
        }
    }
}
