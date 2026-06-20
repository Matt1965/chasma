use std::collections::HashMap;
use std::path::Path;

use calamine::{open_workbook, Data, Reader, Xlsx, XlsxError};

use super::error::{DataImportError, RowImportError};
use super::schema::{
    parse_bool_yn, parse_enabled_cell, DoodadImportRow, REQUIRED_COLUMNS,
};

pub const DOODADS_SHEET_NAME: &str = "Doodads";

/// Map header row cells to column indices by exact trimmed header name.
pub fn column_map_from_headers(headers: &[String]) -> Result<HashMap<String, usize>, DataImportError> {
    let mut map = HashMap::new();
    for (index, header) in headers.iter().enumerate() {
        let key = header.trim();
        if key.is_empty() {
            continue;
        }
        map.entry(key.to_string()).or_insert(index);
    }

    for &required in REQUIRED_COLUMNS {
        if !map.contains_key(required) {
            return Err(DataImportError::MissingRequiredColumn {
                column: required.to_string(),
            });
        }
    }

    Ok(map)
}

pub fn read_doodad_rows(
    path: &Path,
) -> Result<Vec<Result<DoodadImportRow, RowImportError>>, DataImportError> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(DOODADS_SHEET_NAME)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;

    let mut rows = range.rows();
    let header_cells = rows
        .next()
        .ok_or_else(|| DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(parse_row(row_number, cells, &columns).map_err(|message| RowImportError {
            row_number,
            message,
        }));
    }

    Ok(parsed)
}

fn row_is_empty(cells: &[Data]) -> bool {
    cells.iter().all(|cell| cell_to_string(cell).trim().is_empty())
}

fn parse_row(
    row_number: usize,
    cells: &[Data],
    columns: &HashMap<String, usize>,
) -> Result<DoodadImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let float = |column: &str| -> Result<f32, String> {
        let raw = text(column);
        if raw.trim().is_empty() {
            return Err(format!("{column} must be a number"));
        }
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
    };

    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;
    let random_rotation = parse_bool_yn(&text("Random Rotation (Y/N)"))?;

    Ok(DoodadImportRow {
        row_number,
        name: text("Name"),
        description: text("Description"),
        category: text("Category"),
        biome: text("Biome"),
        file_path: text("File Path"),
        min_size: float("Min Size")?,
        max_size: float("Max Size")?,
        spawn_weight: float("Spawn Weight")?,
        random_rotation,
        enabled,
        enabled_was_blank,
    })
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => trim_float(*value),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => {
            if *value {
                "Y".to_string()
            } else {
                "N".to_string()
            }
        }
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(_) => String::new(),
    }
}

fn trim_float(value: f64) -> String {
    let value = value as f32;
    if (value - value.round()).abs() < f32::EPSILON {
        value.round().to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata").join("doodads").join(name)
    }

    fn write_workbook(path: &Path, headers: &[&str], rows: &[Vec<&str>]) {
        use rust_xlsxwriter::Workbook;
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

    #[test]
    fn column_order_is_irrelevant() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "column_order"
        ));
        let headers = [
            "Enabled",
            "Random Rotation (Y/N)",
            "Spawn Weight",
            "Max Size",
            "Min Size",
            "File Path",
            "Biome",
            "Category",
            "Description",
            "Name",
        ];
        let row = vec![
            "Y", "Y", "5", "1.2", "0.8", "tree/oak.glb", "Forest", "Tree", "Oak", "tree_oak",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_doodad_rows(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].as_ref().unwrap().name, "tree_oak");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_column_fails_import() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "missing_column"
        ));
        write_workbook(
            &path,
            &["Name", "Description", "Category"],
            &[vec!["tree_oak", "Oak", "Tree"]],
        );
        let err = read_doodad_rows(&path).unwrap_err();
        assert!(matches!(
            err,
            DataImportError::MissingRequiredColumn { .. }
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_numeric_field_fails_row_parse() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "bad_number"
        ));
        let headers = [
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
        ];
        let row = vec![
            "tree_oak",
            "Oak",
            "Tree",
            "Forest",
            "tree/oak.glb",
            "not_a_number",
            "1.2",
            "5",
            "Y",
            "Y",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_doodad_rows(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deterministic_import_from_same_file() {
        let path = fixture_path("sample_doodads.xlsx");
        if !path.exists() {
            let headers = [
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
            ];
            let rows = vec![
                vec![
                    "tree_oak", "Oak Tree", "Tree", "Forest", "tree/oak.glb", "0.85", "1.15",
                    "8", "Y", "Y",
                ],
                vec![
                    "rock_small", "Small Rock", "Rock", "Forest", "rock/small.glb", "0.8",
                    "1.2", "3", "N", "Y",
                ],
            ];
            write_workbook(&path, &headers, &rows);
        }
        let a = read_doodad_rows(&path).unwrap();
        let b = read_doodad_rows(&path).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn enabled_blank_defaults_true() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "enabled_blank"
        ));
        let headers = [
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
        ];
        let row = vec![
            "tree_oak", "Oak", "Tree", "Forest", "tree/oak.glb", "0.8", "1.2", "5", "Y", "",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_doodad_rows(&path).unwrap();
        let row = rows[0].as_ref().unwrap();
        assert!(row.enabled);
        assert!(row.enabled_was_blank);
        let _ = std::fs::remove_file(path);
    }
}
