//! Numeric parsing, clamping, and draft commit helpers (Slice 9).

/// Result of parsing user numeric input.
#[derive(Debug, Clone, PartialEq)]
pub enum NumericParseResult {
    Intermediate,
    Valid(f32),
    Invalid(String),
}

/// Parse a numeric draft string; intermediate states do not corrupt authoritative values.
pub fn parse_numeric_draft(text: &str, signed: bool, decimal: bool) -> NumericParseResult {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return NumericParseResult::Intermediate;
    }
    if trimmed == "-" && signed {
        return NumericParseResult::Intermediate;
    }
    if trimmed == "." && decimal {
        return NumericParseResult::Intermediate;
    }
    if trimmed.ends_with('.') && decimal {
        if trimmed[..trimmed.len() - 1].parse::<f32>().is_ok() || trimmed == "-." {
            return NumericParseResult::Intermediate;
        }
    }
    match trimmed.parse::<f32>() {
        Ok(v) => NumericParseResult::Valid(v),
        Err(_) => NumericParseResult::Invalid(format!("Cannot parse '{trimmed}' as a number")),
    }
}

/// Clamp or reject out-of-range values. Returns `None` when reject policy applies.
pub fn apply_numeric_bounds(
    value: f32,
    min: Option<f32>,
    max: Option<f32>,
    clamp: bool,
) -> Result<f32, String> {
    if let Some(min) = min {
        if value < min {
            if clamp {
                return Ok(min);
            }
            return Err(format!("Value {value} is below minimum {min}"));
        }
    }
    if let Some(max) = max {
        if value > max {
            if clamp {
                return Ok(max);
            }
            return Err(format!("Value {value} is above maximum {max}"));
        }
    }
    Ok(value)
}

/// Session-local draft for a single numeric field.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NumericDraft {
    pub text: String,
    pub focused: bool,
}

impl NumericDraft {
    pub fn sync_from_authoritative(&mut self, value: f32, precision: usize) {
        if !self.focused {
            self.text = format_numeric_display(value, precision);
        }
    }

    pub fn begin_edit(&mut self, current: f32, precision: usize) {
        self.focused = true;
        self.text = format_numeric_display(current, precision);
    }

    pub fn clear_focus(&mut self, authoritative: f32, precision: usize) {
        self.focused = false;
        self.text = format_numeric_display(authoritative, precision);
    }
}

pub fn format_numeric_display(value: f32, precision: usize) -> String {
    match precision {
        0 => format!("{value:.0}"),
        1 => format!("{value:.1}"),
        2 => format!("{value:.2}"),
        3 => format!("{value:.3}"),
        _ => format!("{value:.4}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intermediate_dash_does_not_parse() {
        assert_eq!(
            parse_numeric_draft("-", true, true),
            NumericParseResult::Intermediate
        );
    }

    #[test]
    fn intermediate_empty_does_not_parse() {
        assert_eq!(
            parse_numeric_draft("", true, true),
            NumericParseResult::Intermediate
        );
    }

    #[test]
    fn valid_parses_float() {
        assert_eq!(
            parse_numeric_draft("1.25", true, true),
            NumericParseResult::Valid(1.25)
        );
    }

    #[test]
    fn invalid_rejects_text() {
        match parse_numeric_draft("abc", true, true) {
            NumericParseResult::Invalid(_) => {}
            other => panic!("expected invalid, got {other:?}"),
        }
    }

    #[test]
    fn clamp_to_max() {
        assert_eq!(apply_numeric_bounds(10.0, None, Some(5.0), true), Ok(5.0));
    }

    #[test]
    fn reject_below_min() {
        assert!(apply_numeric_bounds(1.0, Some(2.0), None, false).is_err());
    }

    #[test]
    fn draft_preserves_intermediate_while_focused() {
        let mut draft = NumericDraft::default();
        draft.begin_edit(3.0, 1);
        draft.text = "-".into();
        draft.sync_from_authoritative(99.0, 1);
        assert_eq!(draft.text, "-");
        draft.clear_focus(3.0, 1);
        assert_eq!(draft.text, "3.0");
    }
}
