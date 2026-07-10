use std::collections::HashMap;

use super::schema::{WeaponImportRow, REQUIRED_COLUMNS};
use crate::data_import::error::{DataImportError, RowImportError};
use crate::data_import::schema::parse_enabled_cell;
use crate::world::{DamageType, HitMode, TargetFilter};

pub const WEAPONS_SHEET_NAME: &str = "Weapons";

pub fn column_map_from_headers(headers: &[String]) -> Result<HashMap<String, usize>, DataImportError> {
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

pub fn read_weapon_rows(
    path: &std::path::Path,
) -> Result<Vec<Result<WeaponImportRow, RowImportError>>, DataImportError> {
    use calamine::{open_workbook, Reader, Xlsx, XlsxError};

    let mut workbook: Xlsx<_> =
        open_workbook(path).map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    let range = workbook
        .worksheet_range(WEAPONS_SHEET_NAME)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: WEAPONS_SHEET_NAME.to_string(),
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
        parsed.push(parse_row(row_number, cells, &columns).map_err(|message| RowImportError {
            row_number,
            message,
        }));
    }

    Ok(parsed)
}

fn row_is_empty(cells: &[calamine::Data]) -> bool {
    cells.iter().all(|cell| cell_to_string(cell).trim().is_empty())
}

fn parse_row(
    row_number: usize,
    cells: &[calamine::Data],
    columns: &HashMap<String, usize>,
) -> Result<WeaponImportRow, String> {
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
    let optional_text = |column: &str| -> Option<String> {
        let raw = text(column);
        if raw.trim().is_empty() {
            None
        } else {
            Some(raw.trim().to_string())
        }
    };

    let optional_f32 = |column: &str| -> Result<Option<f32>, String> {
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

    let (enabled, enabled_was_blank) = parse_enabled_cell(&text("Enabled"))?;
    let projectile_key = optional_text("Projectile Key");
    let stat_scaling = optional_text("Stat Scaling");
    let target_filters = TargetFilter::parse_list(&text("Target Filters"))?;
    let hit_mode = HitMode::parse(&text("Hit Mode"))?;
    let projectile_speed_mps = optional_f32("Projectile Speed")?.unwrap_or(0.0);

    Ok(WeaponImportRow {
        row_number,
        weapon_id: text("Weapon ID"),
        name: text("Name"),
        description: text("Description"),
        damage: f32_col("Damage")?,
        damage_type: DamageType::parse(&text("Damage Type"))?,
        range_meters: f32_col("Range")?,
        attacks_per_second: f32_col("Attacks Per Second")?,
        windup_seconds: f32_col("Windup")?,
        recovery_seconds: f32_col("Recovery")?,
        hit_mode,
        projectile_key,
        projectile_speed_mps,
        animation_key: text("Animation Key"),
        target_filters,
        stat_scaling,
        enabled,
        enabled_was_blank,
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
        sheet.set_name(WEAPONS_SHEET_NAME).unwrap();
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

    fn full_headers() -> Vec<&'static str> {
        vec![
            "Weapon ID",
            "Name",
            "Description",
            "Damage",
            "Damage Type",
            "Range",
            "Attacks Per Second",
            "Windup",
            "Recovery",
            "Hit Mode",
            "Projectile Key",
            "Animation Key",
            "Target Filters",
            "Stat Scaling",
            "Enabled",
        ]
    }

    #[test]
    fn column_order_is_irrelevant() {
        let path = std::env::temp_dir().join(format!(
            "chasma_weapon_import_{}_{}.xlsx",
            std::process::id(),
            "column_order"
        ));
        let headers = vec![
            "Enabled",
            "Stat Scaling",
            "Target Filters",
            "Animation Key",
            "Projectile Key",
            "Hit Mode",
            "Recovery",
            "Windup",
            "Attacks Per Second",
            "Range",
            "Damage Type",
            "Damage",
            "Description",
            "Name",
            "Weapon ID",
        ];
        let row = vec![
            "Y", "", "Enemies", "attack_fists", "", "Melee", "0.1", "0.1", "1.5", "1.2",
            "Blunt", "4", "Unarmed", "Fists", "weapon_fists",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_weapon_rows(&path).unwrap();
        assert_eq!(rows[0].as_ref().unwrap().weapon_id, "weapon_fists");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reads_full_weapon_row() {
        let path = std::env::temp_dir().join(format!(
            "chasma_weapon_import_{}_{}.xlsx",
            std::process::id(),
            "full"
        ));
        let headers = full_headers();
        let row = vec![
            "weapon_fists",
            "Fists",
            "Unarmed",
            "4",
            "Blunt",
            "1.2",
            "1.5",
            "0.15",
            "0.1",
            "Melee",
            "",
            "attack_fists",
            "Enemies",
            "",
            "Y",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_weapon_rows(&path).unwrap();
        let row = rows[0].as_ref().unwrap();
        assert_eq!(row.weapon_id, "weapon_fists");
        assert!((row.attacks_per_second - 1.5).abs() < 1e-4);
        let _ = std::fs::remove_file(path);
    }
}
