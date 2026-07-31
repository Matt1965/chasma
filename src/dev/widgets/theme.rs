//! Shared dev workspace visual language (Slice 9 / 13).

use bevy::prelude::*;

// --- Typography ---

pub const FONT_SIZE_WINDOW_TITLE: f32 = 12.0;
pub const FONT_SIZE_TITLE: f32 = 11.0;
pub const FONT_SIZE_LABEL: f32 = 11.0;
pub const FONT_SIZE_SMALL: f32 = 10.0;
pub const FONT_SIZE_SECTION: f32 = 10.0;
pub const FONT_SIZE_BADGE: f32 = 9.0;

// --- Spacing scale (px) ---

pub const SPACE_TIGHT: f32 = 4.0;
pub const SPACE_CONTROL: f32 = 6.0;
pub const SPACE_SECTION: f32 = 8.0;
pub const SPACE_WINDOW: f32 = 8.0;
pub const SPACE_BUTTON_PAD_X: f32 = 8.0;
pub const SPACE_BUTTON_PAD_Y: f32 = 4.0;

// --- Window chrome ---

pub const WINDOW_BG: Color = Color::srgba(0.04, 0.06, 0.08, 0.92);
pub const WINDOW_BORDER: Color = Color::srgba(0.18, 0.26, 0.32, 0.9);
pub const WINDOW_TITLE_TEXT: Color = Color::srgba(0.35, 0.95, 0.55, 1.0);
pub const LAUNCHER_BG: Color = Color::srgba(0.06, 0.09, 0.12, 0.94);
pub const LAUNCHER_LABEL_TEXT: Color = Color::srgba(0.7, 0.85, 0.92, 1.0);

pub const TITLE_BTN_IDLE: Color = Color::srgba(0.14, 0.22, 0.28, 0.95);
pub const TITLE_BTN_HOVER: Color = Color::srgba(0.22, 0.32, 0.40, 0.98);

// --- Buttons ---

pub const BTN_BG_IDLE: Color = Color::srgba(0.14, 0.22, 0.28, 0.95);
pub const BTN_BG_HOVER: Color = Color::srgba(0.20, 0.30, 0.38, 0.98);
pub const BTN_BG_PRESSED: Color = Color::srgba(0.08, 0.12, 0.16, 1.0);
pub const BTN_BG_ACTIVE: Color = Color::srgba(0.15, 0.45, 0.32, 0.95);
pub const BTN_BG_ON: Color = Color::srgba(0.2, 0.55, 0.35, 0.95);
pub const BTN_BG_ON_HOVER: Color = Color::srgba(0.25, 0.62, 0.42, 0.98);
pub const BTN_BG_DISABLED: Color = Color::srgba(0.10, 0.14, 0.18, 0.75);
pub const BTN_BG_ACCENT: Color = Color::srgba(0.15, 0.40, 0.55, 0.95);

// --- Text ---

pub const TEXT_PRIMARY: Color = Color::srgba(0.88, 0.94, 0.98, 1.0);
pub const TEXT_MUTED: Color = Color::srgba(0.78, 0.84, 0.9, 1.0);
pub const TEXT_SECTION: Color = Color::srgba(0.65, 0.78, 0.88, 1.0);
pub const TEXT_LABEL: Color = Color::srgba(0.7, 0.78, 0.86, 1.0);
pub const TEXT_DISABLED: Color = Color::srgba(0.5, 0.55, 0.6, 0.85);

// --- Fields ---

pub const FIELD_BG_IDLE: Color = Color::srgba(0.08, 0.11, 0.14, 0.95);
pub const FIELD_BG_FOCUSED: Color = Color::srgba(0.10, 0.18, 0.24, 0.98);
pub const FIELD_BORDER_IDLE: Color = Color::srgba(0.25, 0.32, 0.38, 0.9);
pub const FIELD_BORDER_FOCUSED: Color = Color::srgba(0.35, 0.75, 0.55, 1.0);

// --- Status ---

pub const STATUS_INFO: Color = Color::srgba(0.72, 0.82, 0.9, 1.0);
pub const STATUS_SUCCESS: Color = Color::srgba(0.45, 0.85, 0.55, 1.0);
pub const STATUS_WARNING: Color = Color::srgba(0.95, 0.75, 0.35, 1.0);
pub const STATUS_ERROR: Color = Color::srgba(0.95, 0.45, 0.4, 1.0);

// --- Badges ---

pub const BADGE_BG: Color = Color::srgba(0.12, 0.18, 0.24, 0.95);
pub const BADGE_TEXT: Color = Color::srgba(0.75, 0.85, 0.92, 1.0);
pub const BADGE_DIRTY: Color = Color::srgba(0.85, 0.65, 0.25, 1.0);
pub const BADGE_VALID: Color = Color::srgba(0.45, 0.85, 0.55, 1.0);
pub const BADGE_INVALID: Color = Color::srgba(0.95, 0.45, 0.4, 1.0);

// --- Tooltip ---

pub const TOOLTIP_BG: Color = Color::srgba(0.05, 0.08, 0.11, 0.96);
pub const TOOLTIP_TEXT: Color = TEXT_PRIMARY;
pub const TOOLTIP_MAX_WIDTH_PX: f32 = 300.0;
pub const TOOLTIP_PADDING_X: f32 = 8.0;
pub const TOOLTIP_PADDING_Y: f32 = 6.0;

/// Standard menu/action button background from interaction + selected state.
pub fn action_button_bg(interaction: &Interaction, selected: bool) -> BackgroundColor {
    if selected {
        return BackgroundColor(BTN_BG_ACTIVE);
    }
    BackgroundColor(match interaction {
        Interaction::Pressed => BTN_BG_PRESSED,
        Interaction::Hovered => BTN_BG_HOVER,
        Interaction::None => BTN_BG_IDLE,
    })
}

/// Toggle/checkbox on-state styling.
pub fn toggle_button_bg(interaction: &Interaction, on: bool, disabled: bool) -> BackgroundColor {
    if disabled {
        return BackgroundColor(BTN_BG_DISABLED);
    }
    if on {
        BackgroundColor(match interaction {
            Interaction::Pressed => BTN_BG_PRESSED,
            Interaction::Hovered => BTN_BG_ON_HOVER,
            Interaction::None => BTN_BG_ON,
        })
    } else {
        action_button_bg(interaction, false)
    }
}

/// Compact stepper (+/-) button styling.
pub fn stepper_button_bg(interaction: &Interaction, active: bool) -> BackgroundColor {
    if active {
        toggle_button_bg(interaction, true, false)
    } else {
        action_button_bg(interaction, false)
    }
}

/// Title-bar control (close/collapse) styling.
pub fn title_button_bg(interaction: &Interaction) -> BackgroundColor {
    BackgroundColor(match interaction {
        Interaction::Pressed => BTN_BG_PRESSED,
        Interaction::Hovered => TITLE_BTN_HOVER,
        Interaction::None => TITLE_BTN_IDLE,
    })
}

pub fn window_title_font() -> TextFont {
    TextFont {
        font_size: FONT_SIZE_WINDOW_TITLE,
        ..default()
    }
}

pub fn label_text_font() -> TextFont {
    TextFont {
        font_size: FONT_SIZE_LABEL,
        ..default()
    }
}

pub fn small_text_font() -> TextFont {
    TextFont {
        font_size: FONT_SIZE_SMALL,
        ..default()
    }
}

pub fn section_text_font() -> TextFont {
    TextFont {
        font_size: FONT_SIZE_SECTION,
        ..default()
    }
}

pub fn standard_button_node(padding_x: f32, padding_y: f32) -> Node {
    Node {
        padding: UiRect::axes(Val::Px(padding_x), Val::Px(padding_y)),
        ..default()
    }
}

pub fn standard_action_button_node() -> Node {
    standard_button_node(SPACE_BUTTON_PAD_X, SPACE_BUTTON_PAD_Y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_scale_is_monotonic() {
        assert!(SPACE_TIGHT < SPACE_CONTROL);
        assert!(SPACE_CONTROL < SPACE_SECTION);
        assert!(SPACE_SECTION <= SPACE_WINDOW);
    }

    #[test]
    fn status_colors_are_distinct() {
        assert_ne!(STATUS_INFO, STATUS_ERROR);
        assert_ne!(STATUS_SUCCESS, STATUS_WARNING);
    }
}
