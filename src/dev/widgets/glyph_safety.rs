//! Static validation for dev UI strings that must not use unsupported icon glyphs.

/// Unicode symbols that render as missing-glyph boxes with the dev UI font.
///
/// Includes decorative icons and common punctuation substitutes (Unicode minus,
/// ellipsis, em dash, bullets) that the default Bevy UI font does not cover.
pub const FORBIDDEN_DEV_UI_GLYPHS: &[char] = &[
    '▸', '▾', '▶', '▼', '☑', '☐', '✓', '□', '★', //
    '−', // U+2212 MINUS SIGN
    '…', // U+2026 HORIZONTAL ELLIPSIS
    '—', // U+2014 EM DASH
    '•', // U+2022 BULLET
    '⚠', '✗',
];

/// Returns true when `text` contains a forbidden dev UI glyph.
pub fn contains_forbidden_dev_ui_glyph(text: &str) -> bool {
    text.chars().any(|ch| FORBIDDEN_DEV_UI_GLYPHS.contains(&ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_control_labels_are_glyph_safe() {
        for label in [
            "+",
            "-",
            "x",
            "[*]",
            "Save",
            "Cycle enabled",
            "Floor -",
            "Radius -",
            "Regenerate",
            "Save As Variant",
        ] {
            assert!(!contains_forbidden_dev_ui_glyph(label), "{label}");
        }
    }

    #[test]
    fn known_bad_glyphs_are_detected() {
        assert!(contains_forbidden_dev_ui_glyph("▶ collapsed"));
        assert!(contains_forbidden_dev_ui_glyph("★ favorite"));
        assert!(contains_forbidden_dev_ui_glyph("Floor −"));
        assert!(contains_forbidden_dev_ui_glyph("Regenerate…"));
        assert!(contains_forbidden_dev_ui_glyph("Save as variant…"));
    }
}
