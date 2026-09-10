//! Shared Workforce matrix column layout (header and data rows).

use crate::world::WorkPermissionDomain;

pub const PANEL_WIDTH_PX: f32 = 860.0;
pub const PANEL_MIN_WIDTH_PX: f32 = 720.0;

pub const COL_UNIT_WIDTH: f32 = 120.0;
pub const COL_PERMISSION_WIDTH: f32 = 88.0;
pub const COL_GENERAL_LABOR_WIDTH: f32 = 104.0;
pub const COL_CONTROLS_WIDTH: f32 = 132.0;
pub const COL_GAP: f32 = 6.0;

/// Unit + six permission domains + row controls.
pub const MATRIX_COLUMN_COUNT: usize = 8;

pub const CLOSE_BUTTON_LABEL: &str = "X";

pub fn permission_col_width(domain: WorkPermissionDomain) -> f32 {
    match domain {
        WorkPermissionDomain::GeneralLabor => COL_GENERAL_LABOR_WIDTH,
        _ => COL_PERMISSION_WIDTH,
    }
}

pub fn matrix_min_width() -> f32 {
    let permission_sum = WorkPermissionDomain::ALL
        .iter()
        .map(|domain| permission_col_width(*domain))
        .sum::<f32>();
    COL_UNIT_WIDTH
        + permission_sum
        + COL_CONTROLS_WIDTH
        + COL_GAP * (MATRIX_COLUMN_COUNT - 1) as f32
}

pub fn permission_checkbox_label(allowed: bool) -> &'static str {
    if allowed { "[X]" } else { "[ ]" }
}

pub fn forbidden_workforce_ui_characters() -> &'static [char] {
    crate::ui::gameplay::text::FORBIDDEN_UI_GLYPHS
}
