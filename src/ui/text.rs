//! Cross-cutting UI text sync helpers.
//!
//! Typography sizes remain owned by [`crate::menu::font`],
//! [`crate::ui::gameplay::typography`], and [`crate::dev::widgets::theme`].
//! This module owns **mutation discipline** (avoid redundant text/layout writes
//! that churn the shared cosmic-text glyph atlas) and the **default-font-only**
//! [`TextFont`] contract documented in `menu/font.rs`.
//!
//! Do **not** register a second copy of FiraMono / `chasma_menu.ttf` as a
//! separate [`Font`] asset — dual registration produces inconsistently sized
//! glyphs across Dev and menu UI.

use bevy::prelude::*;

/// Explicit absolute [`TextFont`] on Bevy's default UI font.
pub fn absolute_text_font(font_size: f32) -> TextFont {
    TextFont {
        font_size,
        ..default()
    }
}

/// Round UI coordinates to whole pixels to avoid subpixel glyph cache churn.
pub fn snap_ui_px(value: f32) -> f32 {
    value.round()
}

/// Update [`Text`] only when the displayed string actually changes.
pub fn set_text_if_changed(text: &mut Text, value: &str) {
    if text.as_str() != value {
        **text = value.to_string();
    }
}

/// Update [`Text2d`] only when the displayed string actually changes.
pub fn set_text2d_if_changed(text: &mut Text2d, value: &str) {
    if text.as_str() != value {
        *text = Text2d::new(value);
    }
}

/// Assign a pixel [`Val`] only when the snapped value changes.
pub fn set_val_px_if_changed(slot: &mut Val, value: f32) {
    let snapped = snap_ui_px(value);
    if !val_px_matches(slot, snapped) {
        *slot = Val::Px(snapped);
    }
}

fn val_px_matches(slot: &Val, value: f32) -> bool {
    matches!(slot, Val::Px(current) if (*current - value).abs() < 0.01)
}

/// Spawn bundle for a single-style UI label (one [`TextFont`] for all glyphs).
pub fn spawn_ui_label(
    parent: &mut ChildSpawnerCommands<'_>,
    label: impl Into<String>,
    font: TextFont,
    color: Color,
) {
    parent.spawn((Text::new(label.into()), font, TextColor(color)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_text_font_uses_explicit_size_on_default_face() {
        let font = absolute_text_font(12.0);
        assert_eq!(font.font_size, 12.0);
        assert_eq!(absolute_text_font(font.font_size).font_size, 12.0);
    }

    #[test]
    fn set_text_if_changed_skips_identical_content() {
        let mut text = Text::new("hello");
        set_text_if_changed(&mut text, "hello");
        assert_eq!(text.as_str(), "hello");
        set_text_if_changed(&mut text, "world");
        assert_eq!(text.as_str(), "world");
    }

    #[test]
    fn set_val_px_if_changed_avoids_redundant_writes() {
        let mut slot = Val::Px(10.0);
        set_val_px_if_changed(&mut slot, 10.2);
        assert_eq!(slot, Val::Px(10.0));
        set_val_px_if_changed(&mut slot, 11.4);
        assert_eq!(slot, Val::Px(11.0));
    }

    #[test]
    fn spawn_ui_label_bundle_carries_single_text_font() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::ui::UiPlugin));
        let font = absolute_text_font(11.0);
        app.world_mut().spawn_empty().with_children(|parent| {
            spawn_ui_label(parent, "Catalog", font.clone(), Color::WHITE);
        });
        let mut sizes = Vec::new();
        for font in app.world_mut().query::<&TextFont>().iter(app.world()) {
            sizes.push(font.font_size);
        }
        assert_eq!(sizes, vec![11.0]);
    }
}
