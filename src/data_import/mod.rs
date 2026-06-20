//! Excel / RON doodad definition import pipeline (R6).
//!
//! Offline data only: reads authored spreadsheets, validates rows, and produces
//! [`crate::world::DoodadDefinition`] values for [`crate::world::DoodadCatalog`].
//! No ECS systems, runtime Excel dependency, or rendering coupling.

mod error;
mod schema;
mod validate;

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
#[cfg(feature = "data-import")]
mod ron;

pub use error::{DataImportError, RowImportError};
pub use schema::{
    normalize_file_path, normalize_file_path_to_render_key, parse_biome, parse_bool_yn,
    parse_category, parse_enabled_cell, DoodadImportRow, REQUIRED_COLUMNS,
};

#[cfg(feature = "data-import")]
pub use dev_load::{
    resolve_dev_doodad_catalog, DEV_DOODAD_CATALOG_RON_PATH, DEV_DOODAD_EXCEL_PATH,
};
#[cfg(feature = "data-import")]
pub use ron::{export_doodads_to_ron, DoodadCatalogRon, DoodadDefinitionRon};

/// Outcome counters for a doodad Excel import pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub rows_processed: usize,
    pub rows_valid: usize,
    pub rows_failed: usize,
    pub warnings: Vec<String>,
}

/// Import doodad definitions from an Excel workbook (`Doodads` sheet).
#[cfg(feature = "data-import")]
pub fn import_doodads_from_excel(
    path: &std::path::Path,
) -> Result<(Vec<crate::world::DoodadDefinition>, ImportSummary), DataImportError> {
    use std::collections::HashMap;

    use crate::world::DoodadDefinitionId;

    use excel::read_doodad_rows;
    use validate::validate_row;

    let rows = read_doodad_rows(path)?;
    let mut summary = ImportSummary {
        rows_processed: rows.len(),
        ..ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_names: HashMap<DoodadDefinitionId, usize> = HashMap::new();

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

        if let Err(row_err) = validate_row(&row) {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: {}", row_err.row_number, row_err.message));
            continue;
        }

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
        if let Some(first_row) = seen_names.insert(id.clone(), row.row_number) {
            return Err(DataImportError::DuplicateName {
                name: id,
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
        return Err(DataImportError::NoValidRows);
    }

    Ok((definitions, summary))
}

#[cfg(all(feature = "data-import", test))]
mod integration_tests {
    use super::*;
    use crate::world::DoodadKind;
    use excel::DOODADS_SHEET_NAME;
    use rust_xlsxwriter::Workbook;
    use std::path::{Path, PathBuf};

    fn write_workbook(path: &Path, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name(DOODADS_SHEET_NAME).unwrap();
        for (col, header) in headers.iter().enumerate() {
            sheet.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                sheet
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn standard_headers() -> [&'static str; 10] {
        [
            "Name",
            "Description",
            "Category",
            "Biome",
            "File Path",
            "Min Size",
            "Max Size",
            "Spawn Weight",
            "Random Rotation (Y/N)",
            "Enabled",
        ]
    }

    fn temp_workbook(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_import_mod_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    #[test]
    fn import_end_to_end_from_workbook() {
        let path = temp_workbook("e2e");
        let headers = standard_headers();
        let rows = vec![vec![
            "tree_oak", "Oak", "Tree", "Forest", "tree/oak.glb", "0.85", "1.15", "8", "Y", "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_doodads_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id.as_str(), "tree_oak");
        assert_eq!(definitions[0].kind, DoodadKind::Tree);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn disabled_rows_excluded_from_catalog() {
        let path = temp_workbook("disabled");
        let headers = standard_headers();
        let rows = vec![
            vec![
                "tree_oak", "Oak", "Tree", "Forest", "tree/oak.glb", "0.85", "1.15", "8", "Y",
                "Y",
            ],
            vec![
                "tree_dead", "Dead", "Tree", "Forest", "tree/dead.glb", "0.9", "1.1", "2", "Y",
                "N",
            ],
        ];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_doodads_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id.as_str(), "tree_oak");
        assert!(summary
            .warnings
            .iter()
            .any(|w| w.contains("Enabled=false")));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn spawn_weight_preserved_by_import() {
        let path = temp_workbook("weight");
        let headers = standard_headers();
        let rows = vec![vec![
            "tree_oak", "Oak", "Tree", "Forest", "tree/oak.glb", "0.85", "1.15", "12.5", "Y", "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let (definitions, _) = import_doodads_from_excel(&path).unwrap();
        assert!((definitions[0].spawn_weight - 12.5).abs() < 1e-4);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deterministic_import_output() {
        let path = temp_workbook("deterministic");
        let headers = standard_headers();
        let rows = vec![vec![
            "tree_oak", "Oak", "Tree", "Forest", r"\doodads\tree\oak.glb", "0.85", "1.15", "8",
            "Y", "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let a = import_doodads_from_excel(&path).unwrap();
        let b = import_doodads_from_excel(&path).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.0[0].render_key.0.as_deref(), Some("tree/oak"));
        let _ = std::fs::remove_file(path);
    }
}
