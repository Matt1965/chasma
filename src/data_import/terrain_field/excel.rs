use std::collections::HashMap;

use super::schema::{OPTIONAL_COLUMNS, REQUIRED_COLUMNS, TerrainFieldImportRow};
use super::validate::validate_row;
use crate::data_import::ImportSummary;
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;
use crate::world::{TerrainFieldCatalog, TerrainFieldDefinition};

pub const TERRAIN_FIELDS_SHEET_NAME: &str = "Terrain Fields";

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
    let _ = OPTIONAL_COLUMNS;
    Ok(map)
}

pub fn import_terrain_fields_from_excel(
    path: &std::path::Path,
) -> Result<(Vec<TerrainFieldDefinition>, ImportSummary), DataImportError> {
    let rows = read_rows(path)?;
    let mut summary = ImportSummary::default();
    let mut definitions = Vec::new();

    for row_result in rows {
        summary.rows_processed += 1;
        match row_result {
            Ok(row) => match validate_row(&row) {
                Ok(definition) => {
                    summary.rows_valid += 1;
                    definitions.push(definition);
                }
                Err(message) => {
                    summary.rows_failed += 1;
                    summary
                        .warnings
                        .push(format!("row {}: {message}", row.row_number));
                }
            },
            Err(err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {}", err.row_number, err.message));
            }
        }
    }

    Ok((definitions, summary))
}

pub fn import_terrain_field_catalog_from_excel(
    path: &std::path::Path,
) -> Result<(TerrainFieldCatalog, ImportSummary), DataImportError> {
    let (definitions, summary) = import_terrain_fields_from_excel(path)?;
    let catalog = TerrainFieldCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("terrain field catalog build failed: {err}"))
    })?;
    Ok((catalog, summary))
}

fn read_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<TerrainFieldImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(TERRAIN_FIELDS_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: TERRAIN_FIELDS_SHEET_NAME.to_string(),
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
) -> Result<TerrainFieldImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|index| cells.get(*index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let optional_text = |column: &str| -> Option<String> {
        columns
            .get(column)
            .map(|_| text(column))
            .filter(|s| !s.trim().is_empty())
    };
    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;

    Ok(TerrainFieldImportRow {
        row_number,
        field_id: text("Terrain Field ID"),
        name: text("Name"),
        description: text("Description"),
        category: text("Category"),
        value_semantics: text("Value Semantics"),
        enabled,
        enabled_was_blank,
        overlay_enabled: optional_text("Overlay Enabled").map(|value| {
            value.eq_ignore_ascii_case("y")
                || value.eq_ignore_ascii_case("yes")
                || value == "1"
                || value.eq_ignore_ascii_case("true")
        }),
        overlay_low_color: optional_text("Overlay Low Color"),
        overlay_mid_color: optional_text("Overlay Mid Color"),
        overlay_high_color: optional_text("Overlay High Color"),
        overlay_opacity: optional_text("Overlay Opacity").and_then(|value| value.parse().ok()),
        visibility_cutoff: optional_text("Visibility Cutoff").and_then(|value| value.parse().ok()),
        qualitative_thresholds: optional_text("Qualitative Thresholds"),
        qualitative_labels: optional_text("Qualitative Labels"),
        source_profile_id: optional_text("Source Profile ID"),
        icon_key: optional_text("Icon Key"),
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
