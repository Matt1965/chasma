//! Shared search field styling (Slice 9).

pub use super::theme::{FIELD_BG_FOCUSED, FIELD_BG_IDLE, FIELD_BORDER_FOCUSED, FIELD_BORDER_IDLE};

/// Placeholder when catalog search is idle.
pub const CATALOG_SEARCH_PLACEHOLDER: &str = "Search definitions... (Ctrl+F)";

/// Placeholder when scene name field is idle.
pub const SCENE_NAME_PLACEHOLDER: &str = "Scene name... (click or type)";

/// Tooltip for catalog search field.
pub const CATALOG_SEARCH_TOOLTIP: &str = "Filters the current catalog tab by label and id. Enabled-only filter uses E. \
     Ctrl+F focuses this field; typing suppresses global dev shortcuts.";
