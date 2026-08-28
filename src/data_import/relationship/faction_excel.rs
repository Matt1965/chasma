use std::collections::HashMap;

use super::faction_schema::{FACTION_REQUIRED_COLUMNS, FactionImportRow};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;

pub const FACTIONS_SHEET_NAME: &str = "Factions";

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

    for &required in FACTION_REQUIRED_COLUMNS {
        if !map.contains_key(required) {
            return Err(DataImportError::MissingRequiredColumn {
                column: required.to_string(),
            });
        }
    }

    Ok(map)
}

pub fn read_faction_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<FactionImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook.worksheet_range(FACTIONS_SHEET_NAME).map_err(|_| {
        DataImportError::SheetNotFound {
            sheet: FACTIONS_SHEET_NAME.to_string(),
        }
    })?;

    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) || faction_key_cell_is_empty(cells, &columns) {
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

fn faction_key_cell_is_empty(cells: &[calamine::Data], columns: &HashMap<String, usize>) -> bool {
    columns
        .get("Faction Key")
        .and_then(|&index| cells.get(index))
        .map(cell_to_string)
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
}

fn parse_row(
    row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<FactionImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };

    let faction_key = text("Faction Key");
    if faction_key.trim().is_empty() {
        return Err("Faction Key must be non-empty".to_string());
    }

    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;

    Ok(FactionImportRow {
        row_number,
        faction_key,
        name: text("Name"),
        legacy_faction_id: text("Faction ID"),
        description: text("Description"),
        enabled,
        enabled_was_blank,
    })
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
        calamine::Data::DateTime(value) => value.to_string(),
        calamine::Data::DateTimeIso(value) => value.clone(),
        calamine::Data::DurationIso(value) => value.clone(),
        calamine::Data::Error(_) | calamine::Data::Empty => String::new(),
    }
}
