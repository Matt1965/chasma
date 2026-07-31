//! Save window — WorldData snapshot controls (moved from Catalog).

mod panel;

#[cfg(test)]
mod tests;

pub use panel::{
    DevSaveWindowUi, handle_save_window_interaction, setup_save_window_panel,
    sync_dev_save_panel_visibility, sync_save_window_content, sync_save_window_name_field_style,
};
