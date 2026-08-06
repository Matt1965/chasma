//! Shared dev UI widgets (Slice 9).

mod badge;
mod button;
mod confirmation;
mod enum_selector;
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
    DevWidgetActionButton, spawn_action_button, spawn_labeled_stepper_row, spawn_stepper_button,
    sync_action_button_styles,
};
pub use confirmation::{
    DevWidgetConfirmationBar, DevWidgetConfirmationPrompt, set_confirmation_visible,
    spawn_confirmation_bar,
};
pub use enum_selector::{
    DevWidgetSegmentedControl, DevWidgetSegmentedOption, spawn_segmented_control,
    sync_segmented_styles,
};
pub use glyph_safety::{FORBIDDEN_DEV_UI_GLYPHS, contains_forbidden_dev_ui_glyph};
pub use interaction::{
    DevButtonActivationFlash, DevButtonChrome, DevButtonKind, DevButtonVisual, dev_button_visual,
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
    DevStatusSeverity, DevWidgetStatusLine, spawn_status_line, status_text_color,
    sync_status_line_color,
};
pub use theme::{
    BTN_BG_IDLE, CARD_BG, CARD_BORDER, FONT_SIZE_LABEL, SPACE_CONTROL, SPACE_SECTION, SPACE_TIGHT,
    SPACE_WINDOW, TEXT_LABEL, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECTION, WINDOW_BG, WINDOW_TITLE_TEXT,
    action_button_bg, label_text_font, small_text_font, standard_button_node, toggle_button_bg,
};
pub use toggle::{
    DevWidgetToggle, DevWidgetToggleMark, spawn_toggle_row, sync_toggle_styles_with_marker,
};
