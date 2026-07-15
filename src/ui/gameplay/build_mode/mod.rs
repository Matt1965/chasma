//! Player build mode module (ADR-081 B4).

mod catalog;
mod ghost;
mod ghost_scene;
mod input;
mod preview;
mod state;

pub use catalog::{
    BuildCatalogRoot, handle_build_catalog_clicks, handle_build_search_keyboard,
    spawn_build_catalog_panel, sync_build_catalog_contents, sync_build_catalog_visibility,
};
pub use ghost::{BuildGhostStatus, draw_build_mode_ghost};
pub use ghost_scene::{sync_build_mode_ghost_scene, tint_build_mode_ghost_scene};
pub use input::collect_build_mode_intents;
pub use preview::{BuildModeCursorAnchor, update_build_mode_ghost};
pub use state::{BuildModePhase, BuildModeState};
