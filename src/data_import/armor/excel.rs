use std::collections::HashMap;

use super::schema::{ArmorProfileImportRow, REQUIRED_COLUMNS};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;

pub const ARMOR_PROFILES_SHEET_NAME: &str = "Armor Profiles";

pub fn read_armor_profile_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<ArmorProfileImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(ARMOR_PROFILES_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: ARMOR_PROFILES_SHEET_NAME.to_string(),
        })?;

    let mut rows = range.rows();
    let headers: Vec<String> = rows
        .next()
        .ok_or(DataImportError::NoValidRows)?
        .iter()
        .map(cell_to_string)
        .collect();
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

fn column_map_from_headers(headers: &[String]) -> Result<HashMap<String, usize>, DataImportError> {
    let mut map = HashMap::new();
    for (index, header) in headers.iter().enumerate() {
        let key = header.trim();
        if !key.is_empty() {
            map.entry(key.to_string()).or_insert(index);
        }
    }
    for required in REQUIRED_COLUMNS {
        if !map.contains_key(*required) {
            return Err(DataImportError::MissingRequiredColumn {
                column: (*required).to_string(),
            });
        }
    }
    Ok(map)
}

fn parse_armor_rating(raw: &str, row_number: usize) -> Result<u32, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(format!("row {row_number}: Armor Rating must be non-empty"));
    }
    let value = raw
        .parse::<f64>()
        .map_err(|_| format!("row {row_number}: invalid Armor Rating `{raw}`"))?;
    if !value.is_finite() {
        return Err(format!("row {row_number}: Armor Rating must be finite"));
    }
    if value < 0.0 {
        return Err(format!("row {row_number}: Armor Rating must be >= 0"));
    }
    if value.fract() != 0.0 {
        return Err(format!("row {row_number}: Armor Rating must be an integer"));
    }
    Ok(value as u32)
}

fn parse_row(
    row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<ArmorProfileImportRow, String> {
    let text = |name: &str| -> String {
        columns
            .get(name)
            .and_then(|idx| cells.get(*idx))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;
    let armor_rating = parse_armor_rating(&text("Armor Rating"), row_number)?;
    Ok(ArmorProfileImportRow {
        row_number,
        profile_id: text("Profile ID"),
        name: text("Name"),
        description: text("Description"),
        armor_rating,
        enabled,
        enabled_was_blank,
    })
}

fn row_is_empty(cells: &[calamine::Data]) -> bool {
    cells
        .iter()
        .all(|cell| cell_to_string(cell).trim().is_empty())
}

fn cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::String(value) => value.clone(),
        calamine::Data::Float(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        calamine::Data::Int(value) => value.to_string(),
        calamine::Data::Bool(value) => value.to_string(),
        calamine::Data::Empty => String::new(),
        _ => String::new(),
    }
}
