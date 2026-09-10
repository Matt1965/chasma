//! Shared bottom HUD section scaffolding.
//!
//! Sections never size themselves vertically and never draw their own frame:
//! the root owns the continuous frame and the content row owns the vertical
//! bounds. Sections only choose their width and lay out children, which is what
//! keeps every join aligned.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::super::styles::{
    HUD_DIVIDER_COLOR, HUD_PLATE_BG, HUD_PLATE_TRIM, HUD_UTILITY_GROUP_DIVIDER_PX,
};

/// Node for a fixed-width HUD section, stretched to the content row's height.
pub fn hud_section_node(width: f32) -> Node {
    hud_section_node_with_horizontal_padding(
        width,
        super::super::styles::HUD_SECTION_PADDING_X_PX,
        super::super::styles::HUD_SECTION_PADDING_X_PX,
    )
}

/// Fixed-width section with explicit left/right padding (X axis only).
pub fn hud_section_node_with_horizontal_padding(width: f32, left: f32, right: f32) -> Node {
    Node {
        width: Val::Px(width),
        height: Val::Percent(100.0),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect {
            left: Val::Px(left),
            right: Val::Px(right),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
        },
        overflow: Overflow::clip(),
        ..default()
    }
}

/// Node for the flexible HUD section, which absorbs all remaining width.
pub fn hud_flex_section_node(min_width: f32) -> Node {
    Node {
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        min_width: Val::Px(min_width),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect {
            left: Val::Px(super::super::styles::HUD_SECTION_PADDING_X_PX),
            right: Val::Px(super::super::styles::HUD_SECTION_PADDING_X_PX),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
        },
        overflow: Overflow::clip(),
        ..default()
    }
}

/// Horizontal separator between Unit and Settlement utility groups.
pub fn spawn_hud_utility_group_divider(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn((
        Node {
            height: Val::Px(HUD_UTILITY_GROUP_DIVIDER_PX),
            margin: UiRect::vertical(Val::Px(2.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(HUD_DIVIDER_COLOR),
        FocusPolicy::Pass,
    ));
}

/// Fill and trim for the dark inner plates behind portraits, cards, and buttons.
///
/// Corner rounding lives on `Node::border_radius` in Bevy 0.18, so callers set
/// it alongside their own sizing rather than receiving it here.
pub fn hud_plate_colors() -> (BackgroundColor, BorderColor) {
    (
        BackgroundColor(HUD_PLATE_BG),
        BorderColor::all(HUD_PLATE_TRIM),
    )
}
