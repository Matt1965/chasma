use std::collections::HashMap;

use super::schema::{
    BuildingCategoryImportRow, BuildingImportRow, CATEGORY_REQUIRED_COLUMNS, REQUIRED_COLUMNS,
};
use crate::data_import::asset_sizing::{
    BUILDING_TRANSFORM_SAFETY_CLASS, asset_sizing_from_columns, parse_asset_sizing_columns,
};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;
use crate::world::FootprintType;
use crate::world::authoring_transform::BuildingTransformSafetyClass;

pub const BUILDINGS_SHEET_NAME: &str = "Buildings";
pub const BUILDING_CATEGORIES_SHEET_NAME: &str = "Building Categories";

pub fn column_map_from_headers(
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

    for &required in required {
        if !map.contains_key(required) {
            return Err(DataImportError::MissingRequiredColumn {
                column: required.to_string(),
            });
        }
    }

    Ok(map)
}

pub fn read_building_category_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<BuildingCategoryImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(BUILDING_CATEGORIES_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: BUILDING_CATEGORIES_SHEET_NAME.to_string(),
        })?;

    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers, CATEGORY_REQUIRED_COLUMNS)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(
            parse_category_row(row_number, cells, &columns).map_err(|message| RowImportError {
                row_number,
                message,
            }),
        );
    }

    Ok(parsed)
}

pub fn read_building_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<BuildingImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(BUILDINGS_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: BUILDINGS_SHEET_NAME.to_string(),
        })?;

    let mut rows = range.rows();
    let header_cells = rows.next().ok_or(DataImportError::NoValidRows)?;
    let headers: Vec<String> = header_cells.iter().map(cell_to_string).collect();
    let columns = column_map_from_headers(&headers, REQUIRED_COLUMNS)?;

    let mut parsed = Vec::new();
    for (offset, cells) in rows.enumerate() {
        if row_is_empty(cells) {
            continue;
        }
        let row_number = offset + 2;
        parsed.push(
            parse_building_row(row_number, cells, &columns).map_err(|message| RowImportError {
                row_number,
                message,
            }),
        );
    }

    Ok(parsed)
}

fn parse_category_row(
    row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<BuildingCategoryImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };

    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;

    Ok(BuildingCategoryImportRow {
        row_number,
        category_id: text("Category ID"),
        display_name: text("Display Name"),
        description: if columns.contains_key("Description") {
            text("Description")
        } else {
            String::new()
        },
        enabled,
        enabled_was_blank,
    })
}

fn parse_building_row(
    row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<BuildingImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let f32_col = |column: &str| -> Result<f32, String> {
        let raw = text(column);
        if raw.trim().is_empty() {
            return Err(format!("{column} must be a number"));
        }
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
    };
    let optional_f32 = |column: &str| -> Result<Option<f32>, String> {
        if !columns.contains_key(column) {
            return Ok(None);
        }
        let raw = text(column);
        if raw.trim().is_empty() {
            Ok(None)
        } else {
            raw.trim()
                .parse::<f32>()
                .map(Some)
                .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
        }
    };
    let u32_col = |column: &str| -> Result<u32, String> {
        let raw = text(column);
        if raw.trim().is_empty() {
            return Err(format!("{column} must be a positive integer"));
        }
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
            .and_then(|value| {
                if value < 0.0 || (value - value.round()).abs() > f32::EPSILON {
                    Err(format!("{column} must be a positive integer (got `{raw}`)"))
                } else {
                    Ok(value.round() as u32)
                }
            })
    };

    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;
    let footprint_type = FootprintType::parse(&text("Footprint Type"))?;
    let max_slope_degrees =
        if columns.contains_key("Max Slope") && !text("Max Slope").trim().is_empty() {
            f32_col("Max Slope")?
        } else {
            super::schema::DEFAULT_MAX_SLOPE_DEGREES
        };

    Ok(BuildingImportRow {
        row_number,
        building_id: text("Building ID"),
        name: text("Name"),
        category: text("Category"),
        model_file_path: text("Model File Path"),
        collision_file_path: text("Collision File Path"),
        preview_file_path: text("Preview File Path"),
        health: u32_col("Health")?,
        build_time_seconds: f32_col("Build Time")?,
        footprint_type,
        footprint_width_meters: optional_f32("Footprint Width")?,
        footprint_depth_meters: optional_f32("Footprint Depth")?,
        footprint_radius_meters: optional_f32("Footprint Radius")?,
        max_slope_degrees,
        construction_stages: text("Construction Stages"),
        task_provider: text("Task Provider"),
        animation_profile: text("Animation Profile"),
        interaction_profile: text("Interaction Profile"),
        default_space: text("Default Space"),
        inventory_profile_id: text("Inventory Profile ID"),
        has_inventory_profile_column: columns.contains_key("Inventory Profile ID"),
        enabled,
        enabled_was_blank,
        has_collision_file_path_column: columns.contains_key("Collision File Path"),
        has_preview_file_path_column: columns.contains_key("Preview File Path"),
        has_footprint_width_column: columns.contains_key("Footprint Width"),
        has_footprint_depth_column: columns.contains_key("Footprint Depth"),
        has_footprint_radius_column: columns.contains_key("Footprint Radius"),
        asset_sizing: asset_sizing_from_columns(&parse_asset_sizing_columns(
            columns,
            cells,
            &|col| text(col),
        ))
        .unwrap_or_default(),
        transform_safety_class: parse_building_safety_class(&text(BUILDING_TRANSFORM_SAFETY_CLASS)),
        allow_instance_scale: optional_bool(columns, cells, "Allow Instance Scale")?
            .unwrap_or(false),
        min_uniform_instance_scale: optional_f32("Min Uniform Scale")?,
        max_uniform_instance_scale: optional_f32("Max Uniform Scale")?,
    })
}

fn optional_bool(
    columns: &std::collections::HashMap<String, usize>,
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
    crate::data_import::schema::parse_bool_yn(&raw).map(Some)
}

fn parse_building_safety_class(raw: &str) -> BuildingTransformSafetyClass {
    match raw.trim().to_ascii_lowercase().as_str() {
        "decorative" | "decorative_non_navigable" | "non_navigable" => {
            BuildingTransformSafetyClass::DecorativeNonNavigable
        }
        _ => BuildingTransformSafetyClass::Navigable,
    }
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

    fn write_workbook(path: &Path, sheet: &str, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(sheet).unwrap();
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                worksheet
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn write_dual_sheet_workbook(
        path: &Path,
        category_headers: &[&str],
        category_rows: &[Vec<&str>],
        building_headers: &[&str],
        building_rows: &[Vec<&str>],
    ) {
        let mut workbook = Workbook::new();
        let categories = workbook.add_worksheet();
        categories.set_name(BUILDING_CATEGORIES_SHEET_NAME).unwrap();
        for (col, header) in category_headers.iter().enumerate() {
            categories.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in category_rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                categories
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        let buildings = workbook.add_worksheet();
        buildings.set_name(BUILDINGS_SHEET_NAME).unwrap();
        for (col, header) in building_headers.iter().enumerate() {
            buildings.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in building_rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                buildings
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
    fn reads_building_and_category_rows() {
        let path = std::env::temp_dir().join(format!(
            "chasma_building_import_{}_{}.xlsx",
            std::process::id(),
            "read"
        ));
        write_dual_sheet_workbook(
            &path,
            &["Category ID", "Display Name", "Enabled"],
            &[vec!["residential", "Residential", "Y"]],
            &[
                "Building ID",
                "Name",
                "Category",
                "Model File Path",
                "Health",
                "Build Time",
                "Footprint Type",
                "Footprint Width",
                "Footprint Depth",
                "Enabled",
            ],
            &[vec![
                "hut",
                "Hut",
                "residential",
                "hut.glb",
                "100",
                "30",
                "Rectangle",
                "4",
                "4",
                "Y",
            ]],
        );
        let categories = read_building_category_rows(&path).unwrap();
        assert_eq!(categories[0].as_ref().unwrap().category_id, "residential");
        let buildings = read_building_rows(&path).unwrap();
        assert_eq!(buildings[0].as_ref().unwrap().building_id, "hut");
        let _ = std::fs::remove_file(path);
    }
}
