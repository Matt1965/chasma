//! World environment authoring window (Slice 8).

mod panel;

#[cfg(test)]
mod panel_tests;

#[cfg(test)]
mod tests;

pub use panel::{setup_world_window_panel, sync_dev_world_panel_visibility};
