//! Unit definition Excel import (ADR-027 U1).

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;
mod validate;

pub use schema::{
    normalize_file_path_to_render_key, UnitImportRow, DEFAULT_COLLISION_RADIUS_METERS,
    DEFAULT_MAX_SLOPE_DEGREES, DEFAULT_MOVE_SPEED_MPS, IGNORED_COLUMNS, OPTIONAL_COLUMNS,
    REQUIRED_COLUMNS,
};

#[cfg(feature = "data-import")]
pub use dev_load::{resolve_dev_unit_catalog, DEV_UNIT_EXCEL_PATH};
#[cfg(feature = "data-import")]
pub use excel::UNITS_SHEET_NAME;

#[cfg(feature = "data-import")]
pub fn import_units_from_excel(
    path: &std::path::Path,
) -> Result<(Vec<crate::world::UnitDefinition>, crate::data_import::ImportSummary), crate::data_import::DataImportError>
{
    use std::collections::HashMap;

    use crate::world::UnitDefinitionId;

    use excel::read_unit_rows;
    use validate::validate_row;

    let rows = read_unit_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<UnitDefinitionId, usize> = HashMap::new();

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
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(crate::data_import::DataImportError::DuplicateUnitId {
                id,
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

    Ok((definitions, summary))
}

#[cfg(all(feature = "data-import", test))]
mod integration_tests {
    use super::*;
    use crate::world::UnitDefinitionId;
    use excel::UNITS_SHEET_NAME;
    use rust_xlsxwriter::Workbook;
    use std::path::{Path, PathBuf};

    fn write_workbook(path: &Path, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name(UNITS_SHEET_NAME).unwrap();
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

    fn full_headers() -> Vec<&'static str> {
        vec![
            "Unit ID",
            "Name",
            "Faction",
            "Level",
            "Base HP",
            "Strength",
            "Dexterity",
            "Constitution",
            "Agility",
            "Charisma",
            "Intelligence",
            "Total Stats",
            "Power Rating",
            "Tier",
            "File Path",
            "Move Speed",
            "Collision Radius",
            "Max Slope",
            "Enabled",
        ]
    }

    fn temp_workbook(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_unit_mod_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    #[test]
    fn import_end_to_end_preserves_stats() {
        let path = temp_workbook("e2e");
        let headers = full_headers();
        let rows = vec![vec![
            "U-0001",
            "Wolf",
            "Wild",
            "2",
            "5",
            "4",
            "6",
            "3",
            "7",
            "2",
            "3",
            "25",
            "26.5",
            "Elite",
            r"\units\wolf.glb",
            "4.5",
            "0.6",
            "40",
            "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_units_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        let def = &definitions[0];
        assert_eq!(def.id.as_str(), "U-0001");
        assert_eq!(def.strength, 4);
        assert_eq!(def.agility, 7);
        assert_eq!(def.render_key.0.as_deref(), Some("wolf"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn disabled_rows_excluded_from_catalog() {
        let path = temp_workbook("disabled");
        let headers = full_headers();
        let rows = vec![
            vec![
                "U-0001", "Wolf", "Wild", "2", "5", "4", "6", "3", "7", "2", "3", "25", "26.5",
                "Elite", r"\units\wolf.glb", "4.5", "0.6", "40", "Y",
            ],
            vec![
                "U-0002", "Deer", "Wild", "1", "4", "2", "5", "2", "8", "1", "2", "20", "12.0",
                "Common", r"\units\deer.glb", "5.5", "0.5", "30", "N",
            ],
        ];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_units_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id.as_str(), "U-0001");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deterministic_import_output() {
        let path = temp_workbook("deterministic");
        let headers = full_headers();
        let rows = vec![vec![
            "U-0003", "Bandit Scout", "Bandits", "3", "8", "4", "7", "3", "6", "3", "4", "27",
            "31.6", "Elite", r"\units\bandit.glb", "3.8", "0.45", "35", "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let a = import_units_from_excel(&path).unwrap();
        let b = import_units_from_excel(&path).unwrap();
        assert_eq!(a, b);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_unit_id_aborts_import() {
        let path = temp_workbook("duplicate");
        let headers = full_headers();
        let rows = vec![
            vec![
                "U-0001", "Wolf", "Wild", "2", "5", "4", "6", "3", "7", "2", "3", "25", "26.5",
                "Elite", r"\units\wolf.glb", "4.5", "0.6", "40", "Y",
            ],
            vec![
                "U-0001", "Wolf Duplicate", "Wild", "2", "5", "4", "6", "3", "7", "2", "3", "25",
                "26.5", "Elite", r"\units\wolf.glb", "4.5", "0.6", "40", "Y",
            ],
        ];
        write_workbook(&path, &headers, &rows);
        let err = import_units_from_excel(&path).unwrap_err();
        assert!(matches!(
            err,
            crate::data_import::DataImportError::DuplicateUnitId { .. }
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn lookup_by_imported_unit_id() {
        let path = temp_workbook("lookup");
        let headers = full_headers();
        let rows = vec![vec![
            "U-0001", "Wolf", "Wild", "2", "5", "4", "6", "3", "7", "2", "3", "25", "26.5",
            "Elite", r"\units\wolf.glb", "4.5", "0.6", "40", "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let (definitions, _) = import_units_from_excel(&path).unwrap();
        let catalog = crate::world::UnitCatalog::from_definitions(definitions).unwrap();
        assert!(catalog.get(&UnitDefinitionId::new("U-0001")).is_some());
        let _ = std::fs::remove_file(path);
    }
}
