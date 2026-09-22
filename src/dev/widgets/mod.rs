//! Shared dev UI widgets (Slice 9).

mod badge;
mod button;
mod confirmation;
mod glyph_safety;
mod interaction;
mod numeric;
mod search;
mod section;
mod slider;
mod status;
pub mod theme;
mod toggle;

#[cfg(test)]
mod tests;

pub use badge::{DevBadgeKind, DevWidgetBadge, spawn_badge};
pub use button::{
    DevWidgetActionButton, spawn_action_button,
    sync_action_button_styles,
};
pub use confirmation::{
    DevWidgetConfirmationBar, DevWidgetConfirmationPrompt, set_confirmation_visible,
    spawn_confirmation_bar,
};
pub use interaction::{
    DevButtonChrome, DevButtonKind,
    queue_button_activation_flash, sync_dev_button_chrome, tick_dev_button_activation_flashes,
};
pub use numeric::{
    NumericDraft, NumericParseResult, apply_numeric_bounds, format_numeric_display,
    parse_numeric_draft,
};
pub use search::{
    CATALOG_SEARCH_PLACEHOLDER, CATALOG_SEARCH_TOOLTIP, FIELD_BG_FOCUSED, FIELD_BG_IDLE,
    FIELD_BORDER_FOCUSED, FIELD_BORDER_IDLE, SCENE_NAME_PLACEHOLDER,
};
pub use section::{
    DevCollapsibleBody, DevCollapsibleSection, DevCollapsibleSectionId, DevCollapsibleState,
    DevCollapsibleToggleButton, handle_collapsible_toggles, spawn_collapsible_section,
    sync_collapsible_sections,
};
pub use slider::{
    DevSliderDragState, DevWidgetSliderTrack, DevWidgetSliderValue, normalized_to_value,
    slider_normalized_x, spawn_bounded_slider_row, sync_slider_fill, value_to_normalized,
};
pub use status::{
    DevStatusSeverity, DevWidgetStatusLine, spawn_status_line,
    sync_status_line_color,
};
pub use theme::{
    BTN_BG_IDLE, CARD_BG, CARD_BORDER, SPACE_CONTROL, SPACE_SECTION, TEXT_LABEL, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECTION, label_text_font, small_text_font, standard_button_node, toggle_button_bg,
};
pub use toggle::{
    DevWidgetToggle, DevWidgetToggleMark, spawn_toggle_row, sync_toggle_styles_with_marker,
};

#[cfg(test)]
pub use glyph_safety::contains_forbidden_dev_ui_glyph;
