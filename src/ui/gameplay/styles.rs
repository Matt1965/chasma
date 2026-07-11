//! Shared Bevy UI styling for the player HUD (P-UI1).

use bevy::prelude::*;

/// Bottom bar height in pixels.
pub const BOTTOM_BAR_HEIGHT_PX: f32 = 200.0;

/// Horizontal padding inside panels.
pub const PANEL_PADDING_PX: f32 = 10.0;

/// Gap between bottom bar sections.
pub const SECTION_GAP_PX: f32 = 4.0;

pub const BAR_BG: Color = Color::srgba(0.04, 0.06, 0.08, 0.82);
pub const PANEL_BG: Color = Color::srgba(0.06, 0.09, 0.11, 0.88);
pub const TEXT_PRIMARY: Color = Color::srgba(0.92, 0.95, 0.98, 1.0);
pub const TEXT_MUTED: Color = Color::srgba(0.65, 0.72, 0.78, 1.0);
pub const ACCENT_GREEN: Color = Color::srgba(0.35, 0.92, 0.42, 1.0);

pub const CMD_BTN_ENABLED_BG: Color = Color::srgba(0.12, 0.18, 0.24, 0.95);
pub const CMD_BTN_ENABLED_HOVER: Color = Color::srgba(0.18, 0.28, 0.36, 0.98);
pub const CMD_BTN_ENABLED_PRESSED: Color = Color::srgba(0.22, 0.38, 0.48, 1.0);
pub const CMD_BTN_DISABLED_BG: Color = Color::srgba(0.08, 0.1, 0.12, 0.55);
pub const CMD_BTN_BORDER: Color = Color::srgba(0.35, 0.55, 0.65, 0.75);
pub const CMD_BTN_ARMED_BG: Color = Color::srgba(0.15, 0.42, 0.22, 0.98);

pub const SQUAD_ENTRY_BG: Color = Color::srgba(0.14, 0.2, 0.26, 0.9);
pub const SQUAD_ENTRY_SELECTED: Color = Color::srgba(0.18, 0.42, 0.24, 0.95);
pub const SQUAD_ENTRY_HOVER: Color = Color::srgba(0.2, 0.32, 0.38, 0.95);

pub fn hud_title_font() -> TextFont {
    TextFont {
        font_size: 14.0,
        ..default()
    }
}

pub fn hud_body_font() -> TextFont {
    TextFont {
        font_size: 12.0,
        ..default()
    }
}

pub fn command_button_bg(interaction: &Interaction, enabled: bool, armed: bool) -> BackgroundColor {
    if !enabled {
        return BackgroundColor(CMD_BTN_DISABLED_BG);
    }
    if armed {
        return BackgroundColor(CMD_BTN_ARMED_BG);
    }
    BackgroundColor(match *interaction {
        Interaction::Pressed => CMD_BTN_ENABLED_PRESSED,
        Interaction::Hovered => CMD_BTN_ENABLED_HOVER,
        Interaction::None => CMD_BTN_ENABLED_BG,
    })
}
