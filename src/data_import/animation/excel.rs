use std::collections::HashMap;

use super::schema::{
    AnimationProfileImportRow, DEFAULT_LOCOMOTION_REFERENCE_SPEED_MPS, REQUIRED_COLUMNS,
};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;

pub const ANIMATION_PROFILES_SHEET_NAME: &str = "Animation Profiles";

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

pub fn read_animation_profile_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<AnimationProfileImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(ANIMATION_PROFILES_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: ANIMATION_PROFILES_SHEET_NAME.to_string(),
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
) -> Result<AnimationProfileImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let optional_f32 = |column: &str, default: f32| -> Result<f32, String> {
        if !columns.contains_key(column) {
            return Ok(default);
        }
        let raw = text(column);
        if raw.trim().is_empty() {
            return Ok(default);
        }
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
    };

    let (enabled, enabled_was_blank) = if columns.contains_key("Enabled") {
        parse_enabled_cell(&text("Enabled"))?
    } else {
        (true, true)
    };

    Ok(AnimationProfileImportRow {
        row_number,
        profile_id: text("Profile ID"),
        idle_animation: text("Idle Animation"),
        walk_animation: text("Walk Animation"),
        run_animation: text("Run Animation"),
        locomotion_reference_speed_mps: optional_f32(
            "Locomotion Reference Speed",
            DEFAULT_LOCOMOTION_REFERENCE_SPEED_MPS,
        )?,
        enabled,
        enabled_was_blank,
        has_walk_column: columns.contains_key("Walk Animation"),
        has_run_column: columns.contains_key("Run Animation"),
        has_reference_speed_column: columns.contains_key("Locomotion Reference Speed"),
    })
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
