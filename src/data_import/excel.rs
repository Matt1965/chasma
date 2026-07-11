use std::collections::HashMap;

use super::error::{DataImportError, RowImportError};
use super::schema::{
    BIOME_COLUMN, BLOCK_RADIUS_COLUMN, BLOCKS_MOVEMENT_COLUMN, DEFINITION_ID_COLUMN_ALIASES,
    DoodadImportRow, RANDOM_ROTATION_COLUMN_ALIASES, REQUIRED_COLUMNS,
    normalize_doodad_definition_id, parse_bool_yn, parse_enabled_cell,
};

pub const DOODADS_SHEET_NAME: &str = "Doodads";

/// Map header row cells to column indices by exact trimmed header name.
pub fn column_map_from_headers(
    headers: &[String],
) -> Result<HashMap<String, usize>, DataImportError> {
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

    if !RANDOM_ROTATION_COLUMN_ALIASES
        .iter()
        .any(|name| map.contains_key(*name))
    {
        return Err(DataImportError::MissingRequiredColumn {
            column: "Random Rotation".to_string(),
        });
    }

    Ok(map)
}

pub fn read_doodad_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<DoodadImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(DOODADS_SHEET_NAME)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;

    let mut rows = range.rows();
    let header_cells = rows.next().ok_or_else(|| DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(
            parse_row(row_number, cells, &columns).map_err(|message| RowImportError {
                row_number,
                message,
            }),
        );
    }

    Ok(parsed)
}

fn row_is_empty(cells: &[calamine::Data]) -> bool {
    cells
        .iter()
        .all(|cell| cell_to_string(cell).trim().is_empty())
}

fn parse_row(
    row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<DoodadImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let text_any = |names: &[&str]| -> String {
        names
            .iter()
            .find_map(|name| columns.get(*name))
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
    let random_rotation = parse_bool_yn(&text_any(RANDOM_ROTATION_COLUMN_ALIASES))?;
    let blocks_movement = optional_bool(columns, cells, BLOCKS_MOVEMENT_COLUMN)?;
    let block_radius_meters = optional_float(columns, cells, BLOCK_RADIUS_COLUMN)?;
    let name = text("Name");
    let definition_id_raw = text_any(DEFINITION_ID_COLUMN_ALIASES);
    let definition_id = if definition_id_raw.trim().is_empty() {
        normalize_doodad_definition_id(&name)?
    } else {
        normalize_doodad_definition_id(definition_id_raw.trim())?
    };

    Ok(DoodadImportRow {
        row_number,
        name,
        definition_id,
        description: text("Description"),
        category: text("Category"),
        biome: columns
            .get(BIOME_COLUMN)
            .map(|_| text(BIOME_COLUMN))
            .unwrap_or_default(),
        file_path: text("File Path"),
        min_size: float("Min Size")?,
        max_size: float("Max Size")?,
        spawn_weight: float("Spawn Weight")?,
        random_rotation,
        enabled,
        enabled_was_blank,
        blocks_movement,
        block_radius_meters,
    })
}

fn optional_bool(
    columns: &HashMap<String, usize>,
    cells: &[calamine::Data],
    column: &str,
) -> Result<Option<bool>, String> {
    let Some(&index) = columns.get(column) else {
        return Ok(None);
    };
    let raw = cells.get(index).map(cell_to_string).unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(None);
    }
    parse_bool_yn(&raw).map(Some)
}

fn optional_float(
    columns: &HashMap<String, usize>,
    cells: &[calamine::Data],
    column: &str,
) -> Result<Option<f32>, String> {
    let Some(&index) = columns.get(column) else {
        return Ok(None);
    };
    let raw = cells.get(index).map(cell_to_string).unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(None);
    }
    raw.trim()
        .parse::<f32>()
        .map(Some)
        .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
}

fn cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        calamine::Data::String(value) => value.clone(),
        calamine::Data::Float(value) => trim_float(*value),
        calamine::Data::Int(value) => value.to_string(),
        calamine::Data::Bool(value) => {
            if *value {
                "Y".to_string()
            } else {
                "N".to_string()
            }
        }
        calamine::Data::DateTime(value) => value.to_string(),
        calamine::Data::DateTimeIso(value) => value.clone(),
        calamine::Data::DurationIso(value) => value.clone(),
        calamine::Data::Error(_) => String::new(),
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

#[cfg(all(feature = "data-import", test))]
mod tests {
    use super::*;
    use std::path::Path;

    use rust_xlsxwriter::Workbook;

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

    fn standard_headers_v2() -> [&'static str; 9] {
        [
            "Name",
            "Description",
            "Category",
            "File Path",
            "Min Size",
            "Max Size",
            "Spawn Weight",
            "Random Rotation",
            "Enabled",
        ]
    }

    #[test]
    fn reads_random_rotation_column_name() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "random_rotation_col"
        ));
        let headers = standard_headers_v2();
        let row = vec![
            "Basic Tree",
            "Basic",
            "Flora",
            r"\doodads\tree",
            "0.5",
            "1.5",
            "10",
            "Y",
            "Y",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_doodad_rows(&path).unwrap();
        let row = rows[0].as_ref().unwrap();
        assert!(row.random_rotation);
        assert_eq!(row.category, "Flora");
        let _ = std::fs::remove_file(path);
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
            "Category",
            "Description",
            "Name",
        ];
        let row = vec![
            "Y",
            "Y",
            "5",
            "1.2",
            "0.8",
            "tree/oak.glb",
            "Tree",
            "Oak",
            "tree_oak",
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
        assert!(matches!(err, DataImportError::MissingRequiredColumn { .. }));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_numeric_field_fails_row() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "invalid_numeric"
        ));
        let headers = standard_headers_v2();
        let row = vec![
            "tree_oak",
            "Oak",
            "Tree",
            "tree/oak.glb",
            "not-a-number",
            "1.2",
            "5",
            "Y",
            "Y",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_doodad_rows(&path).unwrap();
        assert!(rows[0].is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn random_rotation_supports_yn_variants() {
        for (value, expected) in [("Yes", true), ("No", false), ("0", false), ("1", true)] {
            let path = std::env::temp_dir().join(format!(
                "chasma_doodad_import_{}_{}_{}.xlsx",
                std::process::id(),
                "random_rotation_variant",
                value
            ));
            let headers = standard_headers_v2();
            let row = vec![
                "tree_oak",
                "Oak",
                "Tree",
                "tree/oak.glb",
                "0.8",
                "1.2",
                "5",
                value,
                "Y",
            ];
            write_workbook(&path, &headers, &[row]);
            let rows = read_doodad_rows(&path).unwrap();
            assert_eq!(rows[0].as_ref().unwrap().random_rotation, expected);
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn enabled_blank_defaults_true_in_row() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "enabled_blank"
        ));
        let headers = standard_headers_v2();
        let row = vec![
            "tree_oak",
            "Oak",
            "Tree",
            "tree/oak.glb",
            "0.8",
            "1.2",
            "5",
            "Y",
            "",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_doodad_rows(&path).unwrap();
        let row = rows[0].as_ref().unwrap();
        assert!(row.enabled);
        assert!(row.enabled_was_blank);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reads_blocks_movement_and_block_radius_columns() {
        let path = std::env::temp_dir().join(format!(
            "chasma_doodad_import_{}_{}.xlsx",
            std::process::id(),
            "block_columns"
        ));
        let mut headers: Vec<&str> = standard_headers_v2().to_vec();
        headers.push("Blocks Movement");
        headers.push("Block Radius");
        let row = vec![
            "tree_oak",
            "Oak",
            "Tree",
            "tree/oak.glb",
            "0.8",
            "1.2",
            "5",
            "N",
            "Y",
            "No",
            "6.5",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_doodad_rows(&path).unwrap();
        let row = rows[0].as_ref().unwrap();
        assert_eq!(row.blocks_movement, Some(false));
        assert_eq!(row.block_radius_meters, Some(6.5));
        let def = row.clone().to_definition().unwrap();
        assert!(!def.blocks_movement);
        assert_eq!(def.block_radius_meters, 6.5);
        let _ = std::fs::remove_file(path);
    }
}
