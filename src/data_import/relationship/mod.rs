//! Relationship identity Excel import (ADR-132 Phase 1).

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod faction_excel;
mod faction_schema;
mod matrix_domain;
#[cfg(feature = "data-import")]
mod matrix_excel;
#[cfg(feature = "data-import")]
mod matrix_import;
#[cfg(feature = "data-import")]
mod species_excel;
mod species_schema;

pub use matrix_domain::{MatrixDirection, RelationshipMatrixDomain};
#[cfg(feature = "data-import")]
pub use matrix_import::{
    ImportedRelationshipEdge, RELATIONSHIP_MATRIX_REPORT_PATH, RelationshipMatrixImportSummary,
    RelationshipMatrixSheetSummary, export_relationship_matrix_report_markdown,
    import_authored_relationship_matrices_from_excel, parse_relationship_cell,
};

pub use faction_schema::{FACTION_OPTIONAL_COLUMNS, FACTION_REQUIRED_COLUMNS, FactionImportRow};
pub use species_schema::{SPECIES_OPTIONAL_COLUMNS, SPECIES_REQUIRED_COLUMNS, SpeciesImportRow};

#[cfg(feature = "data-import")]
pub use dev_load::{
    resolve_dev_authored_relationship_catalog, resolve_dev_faction_catalog,
    resolve_dev_species_catalog,
};
#[cfg(feature = "data-import")]
pub use faction_excel::FACTIONS_SHEET_NAME;
#[cfg(feature = "data-import")]
pub use species_excel::SPECIES_SHEET_NAME;

#[cfg(feature = "data-import")]
pub use dev_load::import_relationship_identity_catalogs_from_excel;

/// Normalize a workbook relationship identity key to a stable slug.
pub fn normalize_relationship_key(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("relationship identity key must be non-empty".to_string());
    }
    let key = trimmed.to_ascii_lowercase();
    if !key
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(format!(
            "relationship identity key `{trimmed}` must use lowercase letters, digits, and underscores only"
        ));
    }
    Ok(key)
}

#[cfg(feature = "data-import")]
pub fn import_faction_catalog_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        crate::world::FactionCatalog,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::FactionId;

    use faction_excel::read_faction_rows;

    let rows = read_faction_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<FactionId, usize> = HashMap::new();

    for row_result in rows {
        let row = match row_result {
            Ok(row) => row,
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {}", row_err.row_number, row_err.message));
                continue;
            }
        };

        if !row.enabled {
            summary.warnings.push(format!(
                "row {}: Enabled=false — definition excluded from catalog",
                row.row_number
            ));
            continue;
        }

        let definition = match row.to_definition() {
            Ok(definition) => definition,
            Err(message) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {message}", row.row_number));
                continue;
            }
        };

        let id = definition.id.clone();
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(crate::data_import::DataImportError::DuplicateFactionId {
                id: id.as_str().to_string(),
                first_row,
                duplicate_row: row.row_number,
            });
        }

        if row.enabled_was_blank {
            summary.warnings.push(format!(
                "row {}: Enabled blank — defaulting to true",
                row.row_number
            ));
        }

        definitions.push(definition);
        summary.rows_valid += 1;
    }

    if summary.rows_valid == 0 {
        return Err(crate::data_import::DataImportError::NoValidRows);
    }

    let catalog = crate::world::FactionCatalog::from_definitions(definitions).map_err(|err| {
        crate::data_import::DataImportError::WorkbookOpen(format!(
            "faction catalog build failed: {err}"
        ))
    })?;
    Ok((catalog, summary))
}

#[cfg(feature = "data-import")]
pub fn import_species_catalog_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        crate::world::SpeciesCatalog,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::SpeciesId;

    use species_excel::read_species_rows;

    let rows = read_species_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<SpeciesId, usize> = HashMap::new();

    for row_result in rows {
        let row = match row_result {
            Ok(row) => row,
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {}", row_err.row_number, row_err.message));
                continue;
            }
        };

        if !row.enabled {
            summary.warnings.push(format!(
                "row {}: Enabled=false — definition excluded from catalog",
                row.row_number
            ));
            continue;
        }

        let definition = match row.to_definition() {
            Ok(definition) => definition,
            Err(message) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {message}", row.row_number));
                continue;
            }
        };

        let id = definition.id.clone();
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(crate::data_import::DataImportError::DuplicateSpeciesId {
                id: id.as_str().to_string(),
                first_row,
                duplicate_row: row.row_number,
            });
        }

        if row.enabled_was_blank {
            summary.warnings.push(format!(
                "row {}: Enabled blank — defaulting to true",
                row.row_number
            ));
        }

        definitions.push(definition);
        summary.rows_valid += 1;
    }

    if summary.rows_valid == 0 {
        return Err(crate::data_import::DataImportError::NoValidRows);
    }

    let catalog = crate::world::SpeciesCatalog::from_definitions(definitions).map_err(|err| {
        crate::data_import::DataImportError::WorkbookOpen(format!(
            "species catalog build failed: {err}"
        ))
    })?;
    Ok((catalog, summary))
}

#[cfg(all(feature = "data-import", test))]
mod integration_tests {
    use super::*;
    use faction_excel::FACTIONS_SHEET_NAME;
    use rust_xlsxwriter::Workbook;
    use species_excel::SPECIES_SHEET_NAME;
    use std::path::PathBuf;

    fn write_sheet(path: &std::path::Path, sheet: &str, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(sheet).unwrap();
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                worksheet
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn temp_workbook(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_relationship_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    #[test]
    fn faction_import_reads_slug_key() {
        let path = temp_workbook("faction");
        write_sheet(
            &path,
            FACTIONS_SHEET_NAME,
            &["Faction Key", "Name", "Enabled"],
            &[vec!["wild", "Wild", "Y"]],
        );
        let (catalog, summary) = import_faction_catalog_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert!(catalog.contains(&crate::world::FactionId::new("wild")));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn species_import_reads_slug_key() {
        let path = temp_workbook("species");
        write_sheet(
            &path,
            SPECIES_SHEET_NAME,
            &["Species Key", "Name", "Enabled"],
            &[vec!["cavecrawler", "Cavecrawler", "Y"]],
        );
        let (catalog, summary) = import_species_catalog_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert!(catalog.contains(&crate::world::SpeciesId::new("cavecrawler")));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_faction_key_aborts_import() {
        let path = temp_workbook("faction_dup");
        write_sheet(
            &path,
            FACTIONS_SHEET_NAME,
            &["Faction Key", "Name", "Enabled"],
            &[
                vec!["wild", "Wild", "Y"],
                vec!["wild", "Wild Duplicate", "Y"],
            ],
        );
        let err = import_faction_catalog_from_excel(&path).unwrap_err();
        assert!(matches!(
            err,
            crate::data_import::DataImportError::DuplicateFactionId { .. }
        ));
        let _ = std::fs::remove_file(path);
    }
}
