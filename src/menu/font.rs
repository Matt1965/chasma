//! Absolute menu text style constants (default UI font only).
//!
//! Do **not** register a second copy of FiraMono-subset as a separate [`Font`]
//! asset. That dual-registers the same family in cosmic-text's font DB and was
//! observed to produce inconsistently oversized glyphs in Dev/Pause UI while the
//! player HUD (shaped earlier / on the default face alone) stayed intact.
//!
//! Menu sizes stay inside the HUD/Dev band so we never introduce large atlas
//! entries on the shared default font.

use bevy::prelude::*;

/// Title label (Main Menu / Pause heading).
pub const MENU_TITLE_FONT_SIZE: f32 = 16.0;
/// Secondary heading / page titles.
pub const MENU_HEADING_FONT_SIZE: f32 = 14.0;
/// Primary menu button labels.
pub const MENU_BUTTON_FONT_SIZE: f32 = 14.0;
/// Body / helper copy.
pub const MENU_BODY_FONT_SIZE: f32 = 12.0;
/// Small banner / status lines.
pub const MENU_BANNER_FONT_SIZE: f32 = 12.0;

/// Explicit absolute [`TextFont`] on the default UI font (never multiply an existing size).
pub fn menu_text_font(font_size: f32) -> TextFont {
    TextFont {
        font_size,
        ..default()
    }
}

/// Marker on Pause Menu text entities for style invariants / tests.
#[derive(Component, Debug, Clone, Copy)]
pub struct PauseMenuText;

/// Expected absolute font size for a pause control label (buttons / resume).
pub const PAUSE_CONTROL_FONT_SIZE: f32 = MENU_BUTTON_FONT_SIZE;
