use std::collections::HashMap;

use super::schema::{
    DEFAULT_COLLISION_RADIUS_METERS, DEFAULT_MAX_SLOPE_DEGREES, DEFAULT_MOVE_SPEED_MPS,
    DEFAULT_RENDER_SCALE, REQUIRED_COLUMNS, UnitImportRow,
};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;

pub const UNITS_SHEET_NAME: &str = "Units";

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

pub fn read_unit_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<UnitImportRow, RowImportError>>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range =
        workbook
            .worksheet_range(UNITS_SHEET_NAME)
            .map_err(|_| DataImportError::SheetNotFound {
                sheet: UNITS_SHEET_NAME.to_string(),
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
) -> Result<UnitImportRow, String> {
    let text = |column: &str| -> String {
        columns
            .get(column)
            .and_then(|&index| cells.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
    };
    let u32_col = |column: &str| -> Result<u32, String> {
        let raw = text(column);
        if raw.trim().is_empty() {
            return Err(format!("{column} must be a number"));
        }
        raw.trim()
            .parse::<f32>()
            .map_err(|_| format!("{column} must be a number (got `{raw}`)"))
            .and_then(|v| {
                if v < 0.0 || (v - v.round()).abs() > f32::EPSILON {
                    Err(format!(
                        "{column} must be a non-negative integer (got `{raw}`)"
                    ))
                } else {
                    Ok(v.round() as u32)
                }
            })
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

    let has_default_weapon_column = columns.contains_key("Default Weapon ID");
    let (enabled, enabled_was_blank) = if columns.contains_key("Enabled") {
        parse_enabled_cell(&text("Enabled"))?
    } else {
        (true, true)
    };

    let base_hp = u32_col("Base HP")?;
    let max_hp = if columns.contains_key("Max HP") {
        let raw = text("Max HP");
        if raw.trim().is_empty() {
            base_hp
        } else {
            u32_col("Max HP")?
        }
    } else {
        base_hp
    };

    Ok(UnitImportRow {
        row_number,
        unit_id: text("Unit ID"),
        name: text("Name"),
        faction: text("Faction"),
        level: u32_col("Level")?,
        base_hp,
        max_hp,
        strength: u32_col("Strength")?,
        dexterity: u32_col("Dexterity")?,
        constitution: u32_col("Constitution")?,
        agility: u32_col("Agility")?,
        charisma: u32_col("Charisma")?,
        intelligence: u32_col("Intelligence")?,
        power_rating: f32_col("Power Rating")?,
        tier: text("Tier"),
        file_path: if columns.contains_key("File Path") {
            text("File Path")
        } else {
            String::new()
        },
        move_speed_mps: optional_f32("Move Speed", DEFAULT_MOVE_SPEED_MPS)?,
        collision_radius_meters: optional_f32("Collision Radius", DEFAULT_COLLISION_RADIUS_METERS)?,
        max_slope_degrees: optional_f32("Max Slope", DEFAULT_MAX_SLOPE_DEGREES)?,
        render_scale: optional_f32("Render Scale", DEFAULT_RENDER_SCALE)?,
        default_weapon_id: if has_default_weapon_column {
            text("Default Weapon ID")
        } else {
            String::new()
        },
        enabled,
        enabled_was_blank,
        has_file_path_column: columns.contains_key("File Path"),
        has_default_weapon_column,
        has_render_scale_column: columns.contains_key("Render Scale"),
    })
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
        sheet.set_name(UNITS_SHEET_NAME).unwrap();
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

    fn workbook_headers_with_locomotion() -> Vec<&'static str> {
        vec![
            "Unit ID",
            "Name",
            "Faction",
            "Level",
            "Base HP",
            "Max HP",
            "Strength",
            "Dexterity",
            "Constitution",
            "Agility",
            "Charisma",
            "Intelligence",
            "Total Stats",
            "Power Rating",
            "Tier",
            "Default Weapon ID",
            "File Path",
            "Move Speed",
            "Collision Radius",
            "Max Slope",
            "Enabled",
        ]
    }

    fn workbook_headers_legacy() -> Vec<&'static str> {
        vec![
            "Unit ID",
            "Name",
            "Faction",
            "Level",
            "Base HP",
            "Max HP",
            "Strength",
            "Dexterity",
            "Constitution",
            "Agility",
            "Charisma",
            "Intelligence",
            "Total Stats",
            "Power Rating",
            "Tier",
        ]
    }

    #[test]
    fn column_order_is_irrelevant() {
        let path = std::env::temp_dir().join(format!(
            "chasma_unit_import_{}_{}.xlsx",
            std::process::id(),
            "column_order"
        ));
        let headers = vec![
            "Tier",
            "Default Weapon ID",
            "Max HP",
            "Power Rating",
            "Intelligence",
            "Charisma",
            "Agility",
            "Constitution",
            "Dexterity",
            "Strength",
            "Base HP",
            "Level",
            "Faction",
            "Name",
            "Unit ID",
        ];
        let row = vec![
            "Elite",
            "weapon_wolf_bite",
            "5",
            "26.5",
            "3",
            "2",
            "7",
            "3",
            "6",
            "4",
            "5",
            "2",
            "Wild",
            "Wolf",
            "U-0001",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_unit_rows(&path).unwrap();
        assert_eq!(rows[0].as_ref().unwrap().unit_id, "U-0001");
        assert_eq!(
            rows[0].as_ref().unwrap().default_weapon_id,
            "weapon_wolf_bite"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn legacy_workbook_missing_weapon_column_fails() {
        let path = std::env::temp_dir().join(format!(
            "chasma_unit_import_{}_{}.xlsx",
            std::process::id(),
            "legacy"
        ));
        let headers = workbook_headers_legacy();
        let row = vec![
            "U-0001", "Wolf", "Wild", "2", "5", "4", "6", "3", "7", "2", "3", "25", "26.5", "Elite",
        ];
        write_workbook(&path, &headers, &[row]);
        let err = read_unit_rows(&path).unwrap_err();
        assert!(matches!(
            err,
            crate::data_import::DataImportError::MissingRequiredColumn { .. }
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn total_stats_column_is_ignored() {
        let path = std::env::temp_dir().join(format!(
            "chasma_unit_import_{}_{}.xlsx",
            std::process::id(),
            "total_stats"
        ));
        let headers = workbook_headers_with_locomotion();
        let row = vec![
            "U-0001",
            "Wolf",
            "Wild",
            "2",
            "5",
            "5",
            "4",
            "6",
            "3",
            "7",
            "2",
            "3",
            "999",
            "26.5",
            "Elite",
            "weapon_wolf_bite",
            r"\units\wolf.glb",
            "4.5",
            "0.6",
            "40",
            "Y",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_unit_rows(&path).unwrap();
        let def = rows[0].as_ref().unwrap().to_definition().unwrap();
        assert_eq!(def.strength, 4);
        assert_eq!(def.intelligence, 3);
        let _ = std::fs::remove_file(path);
    }
}
