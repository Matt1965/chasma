//! Layered inset plates and raised controls for the bottom HUD (Slice 2).

use bevy::prelude::*;

use super::super::styles::{
    HUD_ACTIVE_ACCENT, HUD_BUTTON_TOP_HIGHLIGHT, HUD_DISABLED_BORDER, HUD_DISABLED_FACE,
    HUD_PLATE_TRIM, HUD_RAISED_FACE, HUD_RAISED_FACE_ARMED, HUD_RAISED_FACE_HOVER,
    HUD_RAISED_FACE_PRESSED, HUD_RAISED_HIGHLIGHT, HUD_RAISED_HIGHLIGHT_HOVER, HUD_RAISED_SHADOW,
    HUD_RECESSED_CORE, HUD_RECESSED_FACE, HUD_RECESSED_HIGHLIGHT, HUD_RECESSED_SHADOW,
    HUD_ROSTER_SLOT_BORDER, HUD_ROSTER_SLOT_FACE, HUD_ROSTER_SLOT_HOVER_FACE,
    HUD_ROSTER_SLOT_PRIMARY_BORDER, HUD_ROSTER_SLOT_SELECTED_BORDER, HUD_ROSTER_SLOT_SELECTED_FACE,
    HUD_TRACK_FACE, HUD_TRACK_HIGHLIGHT, HUD_TRACK_SHADOW,
};

/// Visual state for a raised HUD button shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudButtonShellState {
    Normal,
    Hover,
    Pressed,
    Disabled,
    Armed,
}

impl HudButtonShellState {
    pub fn from_interaction(interaction: &Interaction, enabled: bool, armed: bool) -> Self {
        if !enabled {
            return Self::Disabled;
        }
        if armed && matches!(interaction, Interaction::None | Interaction::Hovered) {
            return Self::Armed;
        }
        match *interaction {
            Interaction::Pressed => Self::Pressed,
            Interaction::Hovered => Self::Hover,
            Interaction::None => Self::Normal,
        }
    }
}

/// Raised controls read lighter on the upper/left edge and darker lower/right.
pub fn hud_raised_bevel_border(state: HudButtonShellState) -> BorderColor {
    match state {
        HudButtonShellState::Pressed => BorderColor {
            left: HUD_RAISED_SHADOW,
            top: HUD_RAISED_SHADOW,
            right: HUD_RAISED_HIGHLIGHT,
            bottom: HUD_RAISED_HIGHLIGHT,
        },
        HudButtonShellState::Disabled => BorderColor::all(HUD_DISABLED_BORDER),
        HudButtonShellState::Armed => BorderColor {
            left: HUD_ACTIVE_ACCENT,
            top: HUD_ACTIVE_ACCENT,
            right: HUD_RAISED_SHADOW,
            bottom: HUD_RAISED_SHADOW,
        },
        HudButtonShellState::Hover => BorderColor {
            left: HUD_RAISED_HIGHLIGHT_HOVER,
            top: HUD_RAISED_HIGHLIGHT_HOVER,
            right: HUD_RAISED_SHADOW,
            bottom: HUD_RAISED_SHADOW,
        },
        HudButtonShellState::Normal => BorderColor {
            left: HUD_RAISED_HIGHLIGHT,
            top: HUD_RAISED_HIGHLIGHT,
            right: HUD_RAISED_SHADOW,
            bottom: HUD_RAISED_SHADOW,
        },
    }
}

pub fn hud_raised_face_color(state: HudButtonShellState) -> Color {
    match state {
        HudButtonShellState::Disabled => HUD_DISABLED_FACE,
        HudButtonShellState::Armed => HUD_RAISED_FACE_ARMED,
        HudButtonShellState::Pressed => HUD_RAISED_FACE_PRESSED,
        HudButtonShellState::Hover => HUD_RAISED_FACE_HOVER,
        HudButtonShellState::Normal => HUD_RAISED_FACE,
    }
}

/// Shell colors for command and utility buttons.
pub fn hud_button_shell_style(
    interaction: &Interaction,
    enabled: bool,
    armed: bool,
) -> (BackgroundColor, BorderColor) {
    let state = HudButtonShellState::from_interaction(interaction, enabled, armed);
    (
        BackgroundColor(hud_raised_face_color(state)),
        hud_raised_bevel_border(state),
    )
}

/// Recessed wells read cut into the plate: darker upper/left, lighter lower/right.
pub fn hud_recessed_bevel_border() -> BorderColor {
    BorderColor {
        left: HUD_RECESSED_SHADOW,
        top: HUD_RECESSED_SHADOW,
        right: HUD_RECESSED_HIGHLIGHT,
        bottom: HUD_RECESSED_HIGHLIGHT,
    }
}

pub fn hud_recessed_well_style() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_RECESSED_FACE),
        hud_recessed_bevel_border(),
    )
}

pub fn hud_recessed_fill_style() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_RECESSED_CORE),
        BorderColor::all(HUD_RECESSED_SHADOW),
    )
}

/// Outer rim for nested portrait/card wells (recessed edge).
pub fn hud_inset_rim_style() -> (BackgroundColor, BorderColor) {
    hud_recessed_well_style()
}

/// Inner recessed fill sitting inside a rim well.
pub fn hud_inset_fill_style() -> (BackgroundColor, BorderColor) {
    hud_recessed_fill_style()
}

/// Raised command / utility button shell at rest.
pub fn hud_button_depth_style() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_RAISED_FACE),
        hud_raised_bevel_border(HudButtonShellState::Normal),
    )
}

pub fn hud_stat_track_style() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_TRACK_FACE),
        BorderColor {
            left: HUD_TRACK_SHADOW,
            top: HUD_TRACK_SHADOW,
            right: HUD_TRACK_HIGHLIGHT,
            bottom: HUD_TRACK_HIGHLIGHT,
        },
    )
}

/// Low-contrast placeholder for empty roster display positions.
pub fn hud_roster_ghost_slot_style() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_ROSTER_SLOT_FACE.with_alpha(0.28)),
        BorderColor::all(HUD_ROSTER_SLOT_BORDER.with_alpha(0.22)),
    )
}

pub fn hud_roster_card_style(
    selected: bool,
    primary: bool,
    interaction: &Interaction,
) -> (Color, BorderColor) {
    let face = if selected {
        HUD_ROSTER_SLOT_SELECTED_FACE
    } else {
        match *interaction {
            Interaction::Hovered | Interaction::Pressed => HUD_ROSTER_SLOT_HOVER_FACE,
            Interaction::None => HUD_ROSTER_SLOT_FACE,
        }
    };
    let border = if primary {
        BorderColor::all(HUD_ROSTER_SLOT_PRIMARY_BORDER)
    } else if selected {
        BorderColor::all(HUD_ROSTER_SLOT_SELECTED_BORDER)
    } else {
        BorderColor::all(HUD_ROSTER_SLOT_BORDER)
    };
    (face, border)
}

/// Quieter raised treatment for roster navigation arrows.
pub fn hud_quiet_raised_style(alpha: f32) -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_RAISED_FACE.with_alpha(alpha)),
        BorderColor {
            left: HUD_RAISED_HIGHLIGHT.with_alpha(alpha * 0.85),
            top: HUD_RAISED_HIGHLIGHT.with_alpha(alpha * 0.85),
            right: HUD_RAISED_SHADOW.with_alpha(alpha),
            bottom: HUD_RAISED_SHADOW.with_alpha(alpha),
        },
    )
}

/// Thin top-edge highlight drawn inside a raised HUD button node.
pub fn spawn_hud_button_top_highlight(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(2.0),
            right: Val::Px(2.0),
            top: Val::Px(2.0),
            height: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(HUD_BUTTON_TOP_HIGHLIGHT),
        ZIndex(1),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn border_luminance(border: BorderColor) -> (f32, f32) {
        let top = border.top.to_srgba();
        let left = border.left.to_srgba();
        let bottom = border.bottom.to_srgba();
        let right = border.right.to_srgba();
        let top_left = (top.red + left.red) * 0.5;
        let bottom_right = (bottom.red + right.red) * 0.5;
        (top_left, bottom_right)
    }

    #[test]
    fn raised_and_recessed_bevels_face_opposite_directions() {
        let raised = hud_raised_bevel_border(HudButtonShellState::Normal);
        let recessed = hud_recessed_bevel_border();
        let (raised_top_left, raised_bottom_right) = border_luminance(raised);
        let (recessed_top_left, recessed_bottom_right) = border_luminance(recessed);
        assert!(
            raised_top_left > raised_bottom_right,
            "raised controls must be lighter on the upper/left edge"
        );
        assert!(
            recessed_top_left < recessed_bottom_right,
            "recessed wells must be darker on the upper/left edge"
        );
    }

    #[test]
    fn pressed_button_inverts_raised_bevel() {
        let normal = hud_raised_bevel_border(HudButtonShellState::Normal);
        let pressed = hud_raised_bevel_border(HudButtonShellState::Pressed);
        let (normal_top, _) = border_luminance(normal);
        let (pressed_top, _) = border_luminance(pressed);
        assert!(
            pressed_top < normal_top,
            "pressed buttons should collapse the raised highlight"
        );
    }

    #[test]
    fn hud_button_shell_avoids_legacy_cyan_palette() {
        let (bg, border) = hud_button_shell_style(&Interaction::None, true, false);
        let edge = border.left.to_srgba();
        let face = bg.0.to_srgba();
        assert!(
            edge.green > edge.blue,
            "HUD borders should read bronze, not cyan"
        );
        assert!(face.blue <= face.red, "HUD button faces stay warm/neutral");
    }

    #[test]
    fn armed_state_uses_warm_accent_not_green() {
        let border = hud_raised_bevel_border(HudButtonShellState::Armed);
        let edge = border.left.to_srgba();
        assert!(
            edge.red > edge.green,
            "armed accent should be warm bronze/gold"
        );
    }
}
