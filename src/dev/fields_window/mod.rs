//! Terrain fields authoring window (Slice 8).

pub(crate) mod forensics;
mod panel;

#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;

pub use panel::{setup_fields_window_panel, sync_dev_fields_panel_visibility};
