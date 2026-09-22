use std::collections::HashMap;

use super::schema::{
    AppearanceBodyVariantImportRow, AppearanceMorphMappingImportRow, AppearanceParameterImportRow,
    AppearanceProfileImportRow, MORPH_MAPPING_REQUIRED_COLUMNS, PARAMETER_REQUIRED_COLUMNS,
    PROFILE_REQUIRED_COLUMNS, VARIANT_REQUIRED_COLUMNS, parse_morph_mapping_side,
};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;

pub const APPEARANCE_PROFILES_SHEET_NAME: &str = "Appearance Profiles";
pub const APPEARANCE_BODY_VARIANTS_SHEET_NAME: &str = "Appearance Body Variants";
pub const APPEARANCE_PARAMETERS_SHEET_NAME: &str = "Appearance Parameters";
pub const APPEARANCE_MORPH_MAPPINGS_SHEET_NAME: &str = "Appearance Morph Mappings";

fn column_map_from_headers(
    headers: &[String],
    required: &[&str],
) -> Result<HashMap<String, usize>, DataImportError> {
    let mut map = HashMap::new();
    for (index, header) in headers.iter().enumerate() {
        let key = header.trim();
        if key.is_empty() {
            continue;
        }
        map.entry(key.to_string()).or_insert(index);
    }
    for required_column in required {
        if !map.contains_key(*required_column) {
            return Err(DataImportError::MissingRequiredColumn {
                column: required_column.to_string(),
            });
        }
    }
    Ok(map)
}

fn row_is_empty(cells: &[calamine::Data]) -> bool {
    cells
        .iter()
        .all(|cell| cell_to_string(cell).trim().is_empty())
}

fn cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        calamine::Data::String(value) => value.clone(),
        calamine::Data::Float(value) => value.to_string(),
        calamine::Data::Int(value) => value.to_string(),
        calamine::Data::Bool(value) => value.to_string(),
        calamine::Data::Error(_) => String::new(),
        calamine::Data::DateTime(value) => value.to_string(),
        calamine::Data::DateTimeIso(value) => value.clone(),
        calamine::Data::DurationIso(value) => value.clone(),
    }
}

pub fn read_appearance_profile_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<AppearanceProfileImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(APPEARANCE_PROFILES_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: APPEARANCE_PROFILES_SHEET_NAME.to_string(),
        })?;
    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers, PROFILE_REQUIRED_COLUMNS)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(parse_profile_row(row_number, cells, &columns).map_err(|message| {
            RowImportError {
                row_number,
                message,
            }
        }));
    }
    Ok(parsed)
}

pub fn read_appearance_body_variant_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<AppearanceBodyVariantImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(APPEARANCE_BODY_VARIANTS_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: APPEARANCE_BODY_VARIANTS_SHEET_NAME.to_string(),
        })?;
    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers, VARIANT_REQUIRED_COLUMNS)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(parse_variant_row(row_number, cells, &columns).map_err(|message| {
            RowImportError {
                row_number,
                message,
            }
        }));
    }
    Ok(parsed)
}

pub fn read_appearance_parameter_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<AppearanceParameterImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(APPEARANCE_PARAMETERS_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: APPEARANCE_PARAMETERS_SHEET_NAME.to_string(),
        })?;
    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers, PARAMETER_REQUIRED_COLUMNS)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(parse_parameter_row(row_number, cells, &columns).map_err(|message| {
            RowImportError {
                row_number,
                message,
            }
        }));
    }
    Ok(parsed)
}

fn parse_profile_row(
    _row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<AppearanceProfileImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let parse_f32 = |column: &str| -> Result<f32, String> {
        let raw = text(column);
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
    };
    let parse_u32 = |column: &str| -> Result<u32, String> {
        let raw = text(column);
        raw.trim()
            .parse::<u32>()
            .map_err(|_| format!("{column} must be a non-negative integer (got `{raw}`)"))
    };
    let (enabled, _) = parse_enabled_cell(&text("Enabled"))?;
    Ok(AppearanceProfileImportRow {
        row_number: 0,
        profile_id: text("Profile ID"),
        species_id: text("Species ID"),
        enabled,
        schema_version: parse_u32("Schema Version")?,
        height_min: parse_f32("Height Min")?,
        height_max: parse_f32("Height Max")?,
        height_default: parse_f32("Height Default")?,
    })
}

fn parse_variant_row(
    _row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<AppearanceBodyVariantImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let (enabled, _) = parse_enabled_cell(&text("Enabled"))?;
    Ok(AppearanceBodyVariantImportRow {
        row_number: 0,
        profile_id: text("Profile ID"),
        variant_id: text("Variant ID"),
        render_key: text("Render Key"),
        display_name: text("Display Name"),
        enabled,
    })
}

pub fn read_appearance_morph_mapping_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<AppearanceMorphMappingImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(APPEARANCE_MORPH_MAPPINGS_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: APPEARANCE_MORPH_MAPPINGS_SHEET_NAME.to_string(),
        })?;
    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers, MORPH_MAPPING_REQUIRED_COLUMNS)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(parse_morph_mapping_row(row_number, cells, &columns).map_err(|message| {
            RowImportError {
                row_number,
                message,
            }
        }));
    }
    Ok(parsed)
}

fn parse_morph_mapping_row(
    _row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<AppearanceMorphMappingImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let parse_f32 = |column: &str| -> Result<f32, String> {
        let raw = text(column);
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
    };
    let (enabled, _) = parse_enabled_cell(&text("Enabled"))?;
    Ok(AppearanceMorphMappingImportRow {
        row_number: 0,
        profile_id: text("Profile ID"),
        variant_id: text("Variant ID"),
        param_id: text("Param ID"),
        target_name: text("Target Name"),
        side: parse_morph_mapping_side(&text("Side"))?,
        multiplier: parse_f32("Multiplier")?,
        enabled,
    })
}

fn parse_parameter_row(
    _row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<AppearanceParameterImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let parse_f32 = |column: &str| -> Result<f32, String> {
        let raw = text(column);
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
    };
    let parse_u32 = |column: &str| -> Result<u32, String> {
        let raw = text(column);
        raw.trim()
            .parse::<u32>()
            .map_err(|_| format!("{column} must be a non-negative integer (got `{raw}`)"))
    };
    let (enabled, _) = parse_enabled_cell(&text("Enabled"))?;
    Ok(AppearanceParameterImportRow {
        row_number: 0,
        param_id: text("Param ID"),
        profile_id: text("Profile ID"),
        display_name: text("Display Name"),
        category: text("Category"),
        min: parse_f32("Min")?,
        max: parse_f32("Max")?,
        default: parse_f32("Default")?,
        display_order: parse_u32("Display Order")?,
        enabled,
    })
}
