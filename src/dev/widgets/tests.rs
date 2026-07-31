//! Widget integration tests (Slice 9).

use super::numeric::{NumericDraft, NumericParseResult, apply_numeric_bounds, parse_numeric_draft};
use super::section::DevCollapsibleState;
use super::status::DevStatusSeverity;
use crate::dev::tooltip::DevTooltipContent;

#[test]
fn tooltip_content_formats_sections() {
    let content = DevTooltipContent::new("Body text")
        .title("Title")
        .units("meters")
        .shortcut("E");
    let text = content.format();
    assert!(text.contains("Title"));
    assert!(text.contains("Body text"));
    assert!(text.contains("Units: meters"));
    assert!(text.contains("Shortcut: E"));
}

#[test]
fn status_severity_ttl_differs() {
    assert!(DevStatusSeverity::Success.default_ttl_frames() > 0);
    assert_eq!(DevStatusSeverity::Error.default_ttl_frames(), 0);
}

#[test]
fn collapsible_defaults_expanded() {
    let state = DevCollapsibleState::default();
    assert!(state.is_expanded(super::section::DevCollapsibleSectionId::DebugMaster));
}

#[test]
fn numeric_reject_out_of_range() {
    assert!(apply_numeric_bounds(100.0, None, Some(5.0), false).is_err());
}

#[test]
fn numeric_intermediate_dash() {
    assert_eq!(
        parse_numeric_draft("-", true, true),
        NumericParseResult::Intermediate
    );
}

#[test]
fn draft_keeps_typing_while_focused() {
    let mut draft = NumericDraft::default();
    draft.begin_edit(2.5, 1);
    draft.text = "2.".into();
    draft.sync_from_authoritative(9.0, 1);
    assert_eq!(draft.text, "2.");
}
