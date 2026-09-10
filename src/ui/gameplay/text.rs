//! ASCII-safe separators and helpers for player-facing HUD chrome.

/// Title/detail separator (e.g. `Settlement Workforce | Settlement 1`).
pub const UI_TITLE_SEPARATOR: &str = " | ";

/// Status/progress separator (e.g. `Growing | 42%`).
pub const UI_STATUS_SEPARATOR: &str = " | ";

/// Inline list / field separator for HUD summaries.
pub const UI_INLINE_SEPARATOR: &str = " | ";

pub fn format_ui_title(primary: &str, detail: &str) -> String {
    format!("{primary}{UI_TITLE_SEPARATOR}{detail}")
}

pub fn format_ui_status(label: &str, value: &str) -> String {
    format!("{label}{UI_STATUS_SEPARATOR}{value}")
}

/// Non-ASCII UI chrome characters that the HUD font does not reliably render.
pub const FORBIDDEN_UI_GLYPHS: &[char] = &[
    '×', '✓', '☑', '☐', '•', '·', '→', '←', '↔', '—', '–', '│', '◀', '▶', '✗', '✔',
];

pub fn ui_chrome_contains_forbidden_glyph(text: &str) -> bool {
    text.chars().any(|ch| FORBIDDEN_UI_GLYPHS.contains(&ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workforce_title_separator_is_ascii_safe() {
        let title = format_ui_title("Settlement Workforce", "Settlement 1");
        assert!(title.contains("Settlement Workforce | Settlement 1"));
        assert!(!ui_chrome_contains_forbidden_glyph(&title));
    }

    #[test]
    fn farm_status_separator_is_ascii_safe() {
        let status = format_ui_status("Growing", "0%");
        assert_eq!(status, "Growing | 0%");
        assert!(!ui_chrome_contains_forbidden_glyph(&status));
    }
}
