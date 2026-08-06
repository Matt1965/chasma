//! Shared Dev button interaction visuals — hover, press, active, flash (IN-10).

use bevy::prelude::*;

use super::theme::{
    BTN_BG_ACTIVATED, BTN_BG_ACTIVE, BTN_BG_DESTRUCTIVE, BTN_BG_DESTRUCTIVE_HOVER,
    BTN_BG_DESTRUCTIVE_PRESSED, BTN_BG_DISABLED, BTN_BG_HOVER, BTN_BG_IDLE, BTN_BG_PRESSED,
    BTN_BG_PRIMARY, BTN_BG_PRIMARY_HOVER, BTN_BG_PRIMARY_PRESSED, BTN_BORDER_ACTIVE,
    BTN_BORDER_IDLE, BTN_BORDER_PRESSED, TEXT_DISABLED,
};

/// Visual role for a dev button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevButtonKind {
    #[default]
    Normal,
    Primary,
    Secondary,
    Destructive,
}

/// Shared chrome state for dev buttons (hover/press/active/disabled).
#[derive(Component, Debug, Clone, Copy)]
pub struct DevButtonChrome {
    pub kind: DevButtonKind,
    pub disabled: bool,
    pub active: bool,
}

impl Default for DevButtonChrome {
    fn default() -> Self {
        Self {
            kind: DevButtonKind::Normal,
            disabled: false,
            active: false,
        }
    }
}

/// Brief post-click activation highlight (transient, not gameplay state).
#[derive(Component, Debug, Clone, Copy)]
pub struct DevButtonActivationFlash {
    pub until_secs: f32,
}

const ACTIVATION_FLASH_SECS: f32 = 0.15;
const PRESS_INSET_PX: f32 = 1.0;

/// Queue a short activation flash on a button entity.
pub fn queue_button_activation_flash(commands: &mut Commands, entity: Entity, now_secs: f32) {
    commands.entity(entity).insert(DevButtonActivationFlash {
        until_secs: now_secs + ACTIVATION_FLASH_SECS,
    });
}

/// Resolved visual layers for one button frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DevButtonVisual {
    pub background: Color,
    pub border: Color,
    pub text: Option<Color>,
    /// Downward visual nudge for the pressed state, applied as a relative
    /// offset so authored layout margins are left untouched.
    pub press_offset_px: f32,
}

/// Compute button visuals from interaction state.
pub fn dev_button_visual(
    interaction: &Interaction,
    kind: DevButtonKind,
    disabled: bool,
    active: bool,
    flashing: bool,
) -> DevButtonVisual {
    if disabled {
        return DevButtonVisual {
            background: BTN_BG_DISABLED,
            border: BTN_BORDER_IDLE,
            text: Some(TEXT_DISABLED),
            press_offset_px: 0.0,
        };
    }

    if flashing {
        return DevButtonVisual {
            background: BTN_BG_ACTIVATED,
            border: BTN_BORDER_ACTIVE,
            text: None,
            press_offset_px: 0.0,
        };
    }

    if active {
        let (bg_idle, bg_hover, bg_pressed) = kind_palette(kind, true);
        let background = match interaction {
            Interaction::Pressed => bg_pressed,
            Interaction::Hovered => bg_hover,
            Interaction::None => bg_idle,
        };
        return DevButtonVisual {
            background,
            border: BTN_BORDER_ACTIVE,
            text: None,
            press_offset_px: press_inset(interaction),
        };
    }

    let (bg_idle, bg_hover, bg_pressed) = kind_palette(kind, false);
    let background = match interaction {
        Interaction::Pressed => bg_pressed,
        Interaction::Hovered => bg_hover,
        Interaction::None => bg_idle,
    };
    DevButtonVisual {
        background,
        border: if matches!(interaction, Interaction::Pressed) {
            BTN_BORDER_PRESSED
        } else {
            BTN_BORDER_IDLE
        },
        text: None,
        press_offset_px: press_inset(interaction),
    }
}

fn press_inset(interaction: &Interaction) -> f32 {
    if matches!(interaction, Interaction::Pressed) {
        PRESS_INSET_PX
    } else {
        0.0
    }
}

fn kind_palette(kind: DevButtonKind, active: bool) -> (Color, Color, Color) {
    if active {
        return (BTN_BG_ACTIVE, BTN_BG_HOVER, BTN_BG_PRESSED);
    }
    match kind {
        DevButtonKind::Normal | DevButtonKind::Secondary => {
            (BTN_BG_IDLE, BTN_BG_HOVER, BTN_BG_PRESSED)
        }
        DevButtonKind::Primary => (BTN_BG_PRIMARY, BTN_BG_PRIMARY_HOVER, BTN_BG_PRIMARY_PRESSED),
        DevButtonKind::Destructive => (
            BTN_BG_DESTRUCTIVE,
            BTN_BG_DESTRUCTIVE_HOVER,
            BTN_BG_DESTRUCTIVE_PRESSED,
        ),
    }
}

/// Apply shared dev button chrome to all widgets carrying [`DevButtonChrome`].
pub fn sync_dev_button_chrome(
    time: Res<Time>,
    mut commands: Commands,
    mut buttons: Query<
        (
            Entity,
            &Interaction,
            &DevButtonChrome,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Node,
            Option<&DevButtonActivationFlash>,
        ),
        With<Button>,
    >,
    mut text: Query<&mut TextColor, With<Button>>,
) {
    let now = time.elapsed_secs();
    for (entity, interaction, chrome, mut bg, mut border, mut node, flash) in &mut buttons {
        if let Some(flash) = flash {
            if now >= flash.until_secs {
                commands.entity(entity).remove::<DevButtonActivationFlash>();
            }
        }
        let flashing = flash.is_some_and(|f| now < f.until_secs);
        let visual = dev_button_visual(
            interaction,
            chrome.kind,
            chrome.disabled,
            chrome.active,
            flashing,
        );
        *bg = BackgroundColor(visual.background);
        *border = BorderColor::all(visual.border);
        node.top = Val::Px(visual.press_offset_px);
        if let Ok(mut color) = text.get_mut(entity) {
            if let Some(text_color) = visual.text {
                *color = TextColor(text_color);
            }
        }
    }
}

/// Expire activation flashes (runs even when chrome query is empty).
pub fn tick_dev_button_activation_flashes(
    time: Res<Time>,
    mut commands: Commands,
    flashes: Query<(Entity, &DevButtonActivationFlash)>,
) {
    let now = time.elapsed_secs();
    for (entity, flash) in &flashes {
        if now >= flash.until_secs {
            commands.entity(entity).remove::<DevButtonActivationFlash>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_never_shows_pressed_fill() {
        let visual = dev_button_visual(
            &Interaction::Pressed,
            DevButtonKind::Normal,
            true,
            false,
            false,
        );
        assert_eq!(visual.background, BTN_BG_DISABLED);
        assert_eq!(visual.press_offset_px, 0.0);
    }

    #[test]
    fn pressed_shows_inset() {
        let visual = dev_button_visual(
            &Interaction::Pressed,
            DevButtonKind::Normal,
            false,
            false,
            false,
        );
        assert_eq!(visual.press_offset_px, PRESS_INSET_PX);
        assert_eq!(visual.background, BTN_BG_PRESSED);
    }

    #[test]
    fn active_tool_uses_active_border() {
        let visual = dev_button_visual(
            &Interaction::None,
            DevButtonKind::Normal,
            false,
            true,
            false,
        );
        assert_eq!(visual.border, BTN_BORDER_ACTIVE);
        assert_eq!(visual.background, BTN_BG_ACTIVE);
    }

    #[test]
    fn destructive_kind_differs_from_normal() {
        let normal = dev_button_visual(
            &Interaction::None,
            DevButtonKind::Normal,
            false,
            false,
            false,
        );
        let destructive = dev_button_visual(
            &Interaction::None,
            DevButtonKind::Destructive,
            false,
            false,
            false,
        );
        assert_ne!(normal.background, destructive.background);
    }

    #[test]
    fn flash_overrides_hover() {
        let visual = dev_button_visual(
            &Interaction::Hovered,
            DevButtonKind::Normal,
            false,
            false,
            true,
        );
        assert_eq!(visual.background, BTN_BG_ACTIVATED);
    }
}
