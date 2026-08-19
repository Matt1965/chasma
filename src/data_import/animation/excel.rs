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
    let optional_duration_seconds = |column: &str| -> Result<Option<f32>, String> {
        if !columns.contains_key(column) {
            return Ok(None);
        }
        let raw = text(column);
        if raw.trim().is_empty() {
            return Ok(None);
        }
        raw.trim()
            .parse::<f32>()
            .map(Some)
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
        death_animation: text("Death Animation"),
        hit_reaction_animation: text("Hit Reaction Animation"),
        upper_body_split_bone: text("Upper Body Split Bone"),
        turn_left_animation: text("Turn Left Animation"),
        turn_right_animation: text("Turn Right Animation"),
        turn_left_duration_seconds: optional_duration_seconds("Turn Left Duration")?,
        turn_right_duration_seconds: optional_duration_seconds("Turn Right Duration")?,
        has_death_column: columns.contains_key("Death Animation"),
        has_hit_reaction_column: columns.contains_key("Hit Reaction Animation"),
        has_upper_body_split_bone_column: columns.contains_key("Upper Body Split Bone"),
        has_turn_left_column: columns.contains_key("Turn Left Animation"),
        has_turn_right_column: columns.contains_key("Turn Right Animation"),
        has_turn_left_duration_column: columns.contains_key("Turn Left Duration"),
        has_turn_right_duration_column: columns.contains_key("Turn Right Duration"),
    })
}

#[cfg(all(feature = "data-import", test))]
mod tests {
    use super::*;
    use std::path::Path;

    use rust_xlsxwriter::Workbook;

    fn write_workbook(path: &Path, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name(ANIMATION_PROFILES_SHEET_NAME).unwrap();
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

    fn legacy_headers() -> Vec<&'static str> {
        vec![
            "Profile ID",
            "Idle Animation",
            "Walk Animation",
            "Run Animation",
            "Locomotion Reference Speed",
            "Enabled",
        ]
    }

    fn extended_headers() -> Vec<&'static str> {
        vec![
            "Profile ID",
            "Idle Animation",
            "Walk Animation",
            "Run Animation",
            "Locomotion Reference Speed",
            "Enabled",
            "Death Animation",
            "Hit Reaction Animation",
            "Upper Body Split Bone",
            "Turn Left Animation",
            "Turn Right Animation",
            "Turn Left Duration",
            "Turn Right Duration",
        ]
    }

    #[test]
    fn legacy_workbook_without_new_columns_imports() {
        let path = std::env::temp_dir().join(format!(
            "chasma_anim_import_{}_{}.xlsx",
            std::process::id(),
            "legacy"
        ));
        let headers = legacy_headers();
        let row = vec!["humanoid", "Idle", "Walk", "Run", "4", "Y"];
        write_workbook(&path, &headers, &[row]);
        let rows = read_animation_profile_rows(&path).unwrap();
        let profile = rows[0].as_ref().unwrap().to_definition();
        assert_eq!(profile.death_clip, None);
        assert_eq!(profile.turn_left_clip, None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn blank_extended_columns_become_none() {
        let path = std::env::temp_dir().join(format!(
            "chasma_anim_import_{}_{}.xlsx",
            std::process::id(),
            "blank_extended"
        ));
        let headers = extended_headers();
        let row = vec![
            "humanoid", "Idle", "Walk", "Run", "4", "Y", "", "", "", "", "", "", "",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_animation_profile_rows(&path).unwrap();
        let profile = rows[0].as_ref().unwrap().to_definition();
        assert_eq!(profile.death_clip, None);
        assert_eq!(profile.hit_reaction_clip, None);
        assert_eq!(profile.upper_body_split_bone, None);
        assert_eq!(profile.turn_left_duration_seconds, None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn populated_extended_columns_map_to_profile() {
        let path = std::env::temp_dir().join(format!(
            "chasma_anim_import_{}_{}.xlsx",
            std::process::id(),
            "extended"
        ));
        let headers = extended_headers();
        let row = vec![
            "cavecrawler",
            "Idle",
            "CrawlForward",
            "",
            "3.5",
            "Y",
            "Death",
            "GetHit1",
            "",
            "CrawlLeft",
            "CrawlRight",
            "1",
            "1",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_animation_profile_rows(&path).unwrap();
        let profile = rows[0].as_ref().unwrap().to_definition();
        assert_eq!(profile.death_clip.as_deref(), Some("Death"));
        assert_eq!(profile.hit_reaction_clip.as_deref(), Some("GetHit1"));
        assert_eq!(profile.turn_left_clip.as_deref(), Some("CrawlLeft"));
        assert_eq!(profile.turn_right_clip.as_deref(), Some("CrawlRight"));
        assert_eq!(profile.turn_left_duration_seconds, Some(1.0));
        assert_eq!(profile.turn_right_duration_seconds, Some(1.0));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_turn_duration_fails_row_parse() {
        let path = std::env::temp_dir().join(format!(
            "chasma_anim_import_{}_{}.xlsx",
            std::process::id(),
            "bad_duration"
        ));
        let headers = extended_headers();
        let row = vec![
            "cavecrawler",
            "Idle",
            "CrawlForward",
            "",
            "3.5",
            "Y",
            "",
            "",
            "",
            "CrawlLeft",
            "CrawlRight",
            "not-a-number",
            "1",
        ];
        write_workbook(&path, &headers, &[row]);
        let rows = read_animation_profile_rows(&path).unwrap();
        assert!(rows[0].is_err());
        let _ = std::fs::remove_file(path);
    }
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
