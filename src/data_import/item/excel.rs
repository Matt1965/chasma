use std::collections::HashMap;

use super::schema::{ItemImportRow, REQUIRED_COLUMNS};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_bool_yn;
use crate::data_import::schema::parse_enabled_cell;
use crate::world::normalize_tags;

pub const ITEMS_SHEET_NAME: &str = "Items";

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

    Ok(map)
}

pub fn read_item_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<ItemImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range =
        workbook
            .worksheet_range(ITEMS_SHEET_NAME)
            .map_err(|_| DataImportError::SheetNotFound {
                sheet: ITEMS_SHEET_NAME.to_string(),
            })?;

    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
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
) -> Result<ItemImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let optional_text = |column: &str| -> Option<String> {
        let value = text(column);
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    };
    let parse_u8 = |column: &str| -> Result<u8, String> {
        let raw = text(column).trim().to_string();
        raw.parse::<u8>()
            .map_err(|_| format!("invalid {column} `{raw}`"))
    };
    let parse_u32 = |column: &str| -> Result<u32, String> {
        let raw = text(column).trim().to_string();
        raw.parse::<u32>()
            .map_err(|_| format!("invalid {column} `{raw}`"))
    };

    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;
    let stackable = parse_bool_yn(&text("Stackable")).map_err(|err| err.to_string())?;
    let unique_instance_required = if columns.contains_key("Unique Instance Required") {
        parse_bool_yn(&text("Unique Instance Required")).map_err(|err| err.to_string())?
    } else {
        false
    };

    let base_value = if columns.contains_key("Base Value") {
        parse_u32("Base Value")?
    } else {
        1
    };

    Ok(ItemImportRow {
        row_number,
        item_id: text("Item ID"),
        name: text("Name"),
        description: optional_text("Description").unwrap_or_default(),
        category: text("Category"),
        width: parse_u8("Width")?,
        height: parse_u8("Height")?,
        stackable,
        max_stack: parse_u32("Max Stack")?,
        mass_grams: parse_u32("Mass Grams")?,
        base_value,
        render_key: optional_text("Render Key"),
        icon_key: optional_text("Icon Key"),
        tags: normalize_tags(&text("Tags")),
        unique_instance_required,
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
