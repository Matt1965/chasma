#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;
mod validate;

#[cfg(feature = "data-import")]
pub use dev_load::resolve_dev_armor_profile_catalog;
#[cfg(feature = "data-import")]
pub use excel::ARMOR_PROFILES_SHEET_NAME;

#[cfg(feature = "data-import")]
pub fn import_armor_profiles_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        Vec<crate::world::ArmorProfileDefinition>,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::ArmorProfileId;

    use excel::read_armor_profile_rows;
    use validate::validate_row;

    let rows = read_armor_profile_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<ArmorProfileId, usize> = HashMap::new();

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
                "row {}: Enabled=false — profile excluded from catalog",
                row.row_number
            ));
            continue;
        }
        let definition = row.to_definition();
        let id = definition.id.clone();
        if seen_ids.insert(id.clone(), row.row_number).is_some() {
            return Err(crate::data_import::DataImportError::WorkbookOpen(format!(
                "duplicate armor profile id `{}`",
                id.as_str()
            )));
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
mod armor_import_tests {
    use std::path::Path;

    use rust_xlsxwriter::Workbook;

    use super::excel::ARMOR_PROFILES_SHEET_NAME;
    use super::import_armor_profiles_from_excel;

    fn temp_workbook(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_armor_import_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    fn write_armor_workbook(path: &Path, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name(ARMOR_PROFILES_SHEET_NAME).unwrap();
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

    #[test]
    fn blank_armor_rating_fails_import() {
        let path = temp_workbook("blank_rating");
        write_armor_workbook(
            &path,
            &[
                "Profile ID",
                "Name",
                "Description",
                "Armor Rating",
                "Enabled",
            ],
            &[vec!["armor_test", "Test Armor", "Test profile", "", "Y"]],
        );
        let summary = match import_armor_profiles_from_excel(&path) {
            Ok((_, summary)) => summary,
            Err(crate::data_import::DataImportError::NoValidRows) => {
                let _ = std::fs::remove_file(path);
                return;
            }
            Err(err) => panic!("unexpected import error: {err}"),
        };
        assert_eq!(summary.rows_valid, 0);
        assert!(summary.rows_failed >= 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn negative_armor_rating_fails_import() {
        let path = temp_workbook("negative_rating");
        write_armor_workbook(
            &path,
            &[
                "Profile ID",
                "Name",
                "Description",
                "Armor Rating",
                "Enabled",
            ],
            &[vec!["armor_test", "Test Armor", "Test profile", "-1", "Y"]],
        );
        let summary = match import_armor_profiles_from_excel(&path) {
            Ok((_, summary)) => summary,
            Err(crate::data_import::DataImportError::NoValidRows) => {
                let _ = std::fs::remove_file(path);
                return;
            }
            Err(err) => panic!("unexpected import error: {err}"),
        };
        assert_eq!(summary.rows_valid, 0);
        assert!(summary.rows_failed >= 1);
        let _ = std::fs::remove_file(path);
    }
}
