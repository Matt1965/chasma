//! Client-local application menu, loading screen, and pause overlay.
//!
//! Temporary session bridge (this foundation pass):
//! - **New Game** and **Edit Default World** reuse the already-initialized runtime
//!   world. They do **not** copy a default-world template or open a separate file.
//! - **Return to Main Menu** pauses simulation, hides gameplay UI, and keeps
//!   `WorldData` resident in memory (no clear/reload).
//! - No player-save or default-world persistence is connected yet.
//!
//! Menu/session state is never written to [`crate::world::WorldData`] or Dev scenes.

mod font;
mod input_block;
mod loading;
mod loading_ui;
mod main_menu;
mod navigation;
mod pause_menu;
mod plugin;
mod screen;
mod settings;
mod systems;
mod transition;

#[cfg(test)]
mod tests;

pub use font::{
    MENU_BANNER_FONT_SIZE, MENU_BODY_FONT_SIZE, MENU_BUTTON_FONT_SIZE, MENU_HEADING_FONT_SIZE,
    MENU_TITLE_FONT_SIZE, PauseMenuText, menu_text_font,
};
pub use input_block::{MenuInputBlock, menu_blocks_input};
pub use loading::{LoadingPhase, LoadingSession};
pub use main_menu::{
    MAIN_MENU_BACKGROUND_PATH, MainMenuBackgroundImage, MainMenuBackgroundOverlay,
};
pub use navigation::{MenuContext, MenuNavigation, MenuPage, PauseMenuContext, pause_menu_is_open};
pub use plugin::{MenuInputSystems, MenuPlugin, MenuUiSystems};
pub use screen::{AppScreen, GameSessionKind, GameSessionState};
pub use settings::{SettingsCategory, SettingsHostKind, SettingsMenuState};
pub use transition::{SessionTransitionKind, SessionTransitionRequest};
