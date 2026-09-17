use std::collections::HashMap;

use super::schema::{EquipmentVisualImportRow, OPTIONAL_COLUMNS, REQUIRED_COLUMNS};
use crate::data_import::error::{DataImportError, RowImportError};

pub const EQUIPMENT_VISUALS_SHEET_NAME: &str = "Equipment Visuals";

pub fn read_equipment_visual_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<EquipmentVisualImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(EQUIPMENT_VISUALS_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: EQUIPMENT_VISUALS_SHEET_NAME.to_string(),
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
    for optional in OPTIONAL_COLUMNS {
        let next_index = map.len();
        map.entry((*optional).to_string()).or_insert(next_index);
    }
    Ok(map)
}

fn parse_row(
    row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<EquipmentVisualImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|index| cells.get(*index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let optional = |column: &str| -> Option<String> {
        let value = text(column);
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    };
    if text("Item ID").trim().is_empty() {
        return Err("Item ID must be non-empty".to_string());
    }
    if text("Unit Render Key").trim().is_empty() {
        return Err("Unit Render Key must be non-empty".to_string());
    }
    if text("Equipped Render Key").trim().is_empty() {
        return Err("Equipped Render Key must be non-empty".to_string());
    }
    if text("Presentation Mode").trim().is_empty() {
        return Err("Presentation Mode must be non-empty".to_string());
    }
    Ok(EquipmentVisualImportRow {
        row_number,
        item_id: text("Item ID"),
        unit_render_key: text("Unit Render Key"),
        equipped_render_key: text("Equipped Render Key"),
        presentation_mode: text("Presentation Mode"),
        socket: optional("Socket"),
        local_translation: optional("Local Translation"),
        local_rotation: optional("Local Rotation"),
        local_scale: optional("Local Scale"),
        stowed_socket: optional("Stowed Socket"),
        stowed_local_translation: optional("Stowed Local Translation"),
        stowed_local_rotation: optional("Stowed Local Rotation"),
        stowed_local_scale: optional("Stowed Local Scale"),
        consumed_morph_params: optional("Consumed Morph Params"),
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
        calamine::Data::Float(value) => value.to_string(),
        calamine::Data::Int(value) => value.to_string(),
        calamine::Data::Bool(value) => value.to_string(),
        calamine::Data::DateTime(value) => value.to_string(),
        calamine::Data::DateTimeIso(value) => value.clone(),
        calamine::Data::DurationIso(value) => value.clone(),
        calamine::Data::Error(_) | calamine::Data::Empty => String::new(),
    }
}
