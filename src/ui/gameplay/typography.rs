//! Gameplay UI typography authority.
//!
//! **Panel typography** (`panel_*`) is the known-good default for floating gameplay
//! windows (Building Menu, inventory, workforce, unit skills, build catalog).
//!
//! **HUD typography** (`hud_*`) is local to the bottom player HUD band only.
//! Do not use `hud_*` helpers outside bottom-HUD modules — they use larger mockup
//! sizes and must not redefine typography consumed by unrelated UI.

use bevy::prelude::*;

// --- Floating gameplay panels (known-good pre-HUD-reskin sizes) ------------

pub const PANEL_BODY_FONT_SIZE: f32 = 12.0;
pub const PANEL_TITLE_FONT_SIZE: f32 = 14.0;

pub fn panel_body_font() -> TextFont {
    TextFont {
        font_size: PANEL_BODY_FONT_SIZE,
        ..default()
    }
}

pub fn panel_title_font() -> TextFont {
    TextFont {
        font_size: PANEL_TITLE_FONT_SIZE,
        ..default()
    }
}

// --- Bottom HUD band only (mockup-proportioned; larger than panels) --------

pub const HUD_HEADING_FONT_SIZE: f32 = 20.0;
pub const HUD_TITLE_FONT_SIZE: f32 = 15.0;
pub const HUD_BODY_FONT_SIZE: f32 = 13.0;
pub const HUD_CAPTION_FONT_SIZE: f32 = 11.0;

pub fn hud_heading_font() -> TextFont {
    TextFont {
        font_size: HUD_HEADING_FONT_SIZE,
        ..default()
    }
}

pub fn hud_title_font() -> TextFont {
    TextFont {
        font_size: HUD_TITLE_FONT_SIZE,
        ..default()
    }
}

pub fn hud_body_font() -> TextFont {
    TextFont {
        font_size: HUD_BODY_FONT_SIZE,
        ..default()
    }
}

pub fn hud_caption_font() -> TextFont {
    TextFont {
        font_size: HUD_CAPTION_FONT_SIZE,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{
        MENU_BODY_FONT_SIZE, MENU_BUTTON_FONT_SIZE, MENU_TITLE_FONT_SIZE, menu_text_font,
    };

    #[test]
    fn panel_typography_matches_known_good_pre_hud_values() {
        assert_eq!(PANEL_BODY_FONT_SIZE, 12.0);
        assert_eq!(PANEL_TITLE_FONT_SIZE, 14.0);
        assert_eq!(panel_body_font().font_size, 12.0);
        assert_eq!(panel_title_font().font_size, 14.0);
    }

    #[test]
    fn hud_typography_is_local_and_larger_than_panel_defaults() {
        assert!(HUD_HEADING_FONT_SIZE > PANEL_TITLE_FONT_SIZE);
        assert!(HUD_TITLE_FONT_SIZE > PANEL_TITLE_FONT_SIZE);
        assert!(HUD_BODY_FONT_SIZE > PANEL_BODY_FONT_SIZE);
        assert_eq!(hud_heading_font().font_size, HUD_HEADING_FONT_SIZE);
        assert_eq!(hud_title_font().font_size, HUD_TITLE_FONT_SIZE);
        assert_eq!(hud_body_font().font_size, HUD_BODY_FONT_SIZE);
        assert_eq!(hud_caption_font().font_size, HUD_CAPTION_FONT_SIZE);
    }

    #[test]
    fn hud_font_helpers_do_not_mutate_panel_defaults() {
        let panel_before = panel_body_font();
        let _ = hud_heading_font();
        let _ = hud_title_font();
        let _ = hud_body_font();
        assert_eq!(panel_body_font(), panel_before);
        assert_eq!(panel_title_font().font_size, PANEL_TITLE_FONT_SIZE);
    }

    #[test]
    fn dev_ui_typography_constants_unchanged() {
        let theme = include_str!("../../dev/widgets/theme.rs");
        assert!(theme.contains("pub const FONT_SIZE_LABEL: f32 = 11.0;"));
        assert!(theme.contains("pub const FONT_SIZE_WINDOW_TITLE: f32 = 12.0;"));
    }

    #[test]
    fn pause_menu_typography_constants_unchanged() {
        assert_eq!(MENU_TITLE_FONT_SIZE, 16.0);
        assert_eq!(MENU_BUTTON_FONT_SIZE, 14.0);
        assert_eq!(MENU_BODY_FONT_SIZE, 12.0);
        assert_eq!(menu_text_font(MENU_BODY_FONT_SIZE).font_size, 12.0);
    }

    #[test]
    fn gameplay_ui_plugin_does_not_touch_ui_scale() {
        let source = include_str!("plugin.rs");
        assert!(
            !source.contains("UiScale"),
            "bottom HUD plugin must not insert or mutate global UiScale"
        );
    }
}
