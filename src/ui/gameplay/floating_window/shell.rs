//! Shared Tier-2 floating gameplay window chrome (Slice 4).

use bevy::prelude::*;

use super::components::FloatingWindowTitleBarDragRegion;
use super::id::FloatingGameplayWindowId;
use super::math::TITLE_BAR_HEIGHT_PX;
use super::tokens::{
    WINDOW_BG, WINDOW_BODY_PADDING_PX, WINDOW_BORDER, WINDOW_BORDER_PX, WINDOW_CLOSE_BUTTON_MIN_PX,
    WINDOW_CORNER_RADIUS_PX, WINDOW_GROOVE_PX, WINDOW_INNER_GROOVE, WINDOW_ROW_SEPARATOR,
    WINDOW_SECTION_BG, WINDOW_SECTION_PADDING_PX, WINDOW_TITLE_ACCENT, WINDOW_TITLE_BG,
};
use crate::ui::gameplay::hud::{
    HudButtonShellState, hud_button_shell_style, hud_raised_bevel_border, hud_raised_face_color,
    hud_recessed_bevel_border,
};
use crate::ui::gameplay::styles::{TEXT_MUTED, TEXT_PRIMARY, panel_body_font, panel_title_font};

/// Marker on the outer gameplay floating-window shell (bronze frame + groove).
#[derive(Component, Debug)]
pub struct FloatingWindowChrome;

/// Title rail above the scrollable body.
#[derive(Component, Debug)]
pub struct FloatingWindowTitleRail;

/// Scrollable / flexible body region below the title rail.
#[derive(Component, Debug)]
pub struct FloatingWindowBody;

/// Shared raised-button chrome for floating-window controls.
#[derive(Component, Debug)]
pub struct FloatingWindowRaisedButton;

/// Close controls spawned through [`spawn_floating_close_button`].
#[derive(Component, Debug)]
pub struct FloatingWindowCloseChrome;

const TITLE_RAIL_HORIZONTAL_PADDING_PX: f32 = 8.0;
const TITLE_RAIL_VERTICAL_PADDING_PX: f32 = 4.0;

/// Outer shell node shared by all gameplay floating windows.
pub fn floating_window_shell_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        flex_direction: FlexDirection::Column,
        overflow: Overflow::clip(),
        border: UiRect::all(Val::Px(WINDOW_BORDER_PX)),
        border_radius: BorderRadius::all(Val::Px(WINDOW_CORNER_RADIUS_PX)),
        display: Display::None,
        ..default()
    }
}

pub fn floating_window_shell_colors() -> (BackgroundColor, BorderColor) {
    (BackgroundColor(WINDOW_BG), BorderColor::all(WINDOW_BORDER))
}

/// Inner groove wrapping the title rail and body.
pub fn spawn_floating_window_inner_frame(
    parent: &mut ChildSpawnerCommands<'_>,
    content: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    parent
        .spawn((
            FloatingWindowChrome,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(WINDOW_GROOVE_PX)),
                border: UiRect::all(Val::Px(WINDOW_GROOVE_PX)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(WINDOW_BG),
            BorderColor::all(WINDOW_INNER_GROOVE),
        ))
        .with_children(content);
}

/// Title rail with drag region and optional close control on the right.
pub fn spawn_floating_title_rail<M: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    window_id: FloatingGameplayWindowId,
    title: impl FnOnce(&mut ChildSpawnerCommands<'_>),
    close: Option<(M, &'static str)>,
) {
    parent
        .spawn((
            FloatingWindowTitleRail,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(WINDOW_TITLE_BG),
        ))
        .with_children(|rail| {
            rail.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::new(
                    Val::Px(TITLE_RAIL_HORIZONTAL_PADDING_PX),
                    Val::Px(TITLE_RAIL_HORIZONTAL_PADDING_PX),
                    Val::Px(TITLE_RAIL_VERTICAL_PADDING_PX),
                    Val::Px(TITLE_RAIL_VERTICAL_PADDING_PX),
                ),
                min_height: Val::Px(TITLE_BAR_HEIGHT_PX),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    FloatingWindowTitleBarDragRegion { id: window_id },
                    Button,
                    Node {
                        flex_grow: 1.0,
                        min_height: Val::Px(
                            TITLE_BAR_HEIGHT_PX - TITLE_RAIL_VERTICAL_PADDING_PX * 2.0,
                        ),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(title);
                if let Some((marker, label)) = close {
                    spawn_floating_close_button(row, marker, label);
                }
            });
            rail.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(WINDOW_TITLE_ACCENT),
            ));
        });
}

/// Full-width drag-only title rail (Building Menu).
pub fn spawn_floating_title_rail_drag_only(
    parent: &mut ChildSpawnerCommands<'_>,
    window_id: FloatingGameplayWindowId,
    title: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    spawn_floating_title_rail::<FloatingWindowCloseChrome>(parent, window_id, title, None);
}

/// Flexible body region beneath the title rail.
pub fn spawn_floating_window_body(
    parent: &mut ChildSpawnerCommands<'_>,
    content: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    parent
        .spawn((
            FloatingWindowBody,
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(WINDOW_BODY_PADDING_PX)),
                row_gap: Val::Px(super::tokens::WINDOW_SECTION_GAP_PX),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .with_children(content);
}

/// Compact raised close control using the HUD button shell at lower relief.
pub fn spawn_floating_close_button<M: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    marker: M,
    label: &str,
) {
    let (bg, border) = hud_button_shell_style(&Interaction::None, true, false);
    parent
        .spawn((
            marker,
            FloatingWindowCloseChrome,
            FloatingWindowRaisedButton,
            Button,
            Node {
                min_width: Val::Px(WINDOW_CLOSE_BUTTON_MIN_PX),
                min_height: Val::Px(WINDOW_CLOSE_BUTTON_MIN_PX),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            bg,
            border,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                panel_title_font(),
                TextColor(if label == "X" {
                    TEXT_MUTED
                } else {
                    TEXT_PRIMARY
                }),
            ));
        });
}

/// Raised action button for floating-window interiors.
pub fn spawn_floating_raised_button<M: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    marker: M,
    label: &str,
    node: Node,
) {
    let (bg, border) = hud_button_shell_style(&Interaction::None, true, false);
    parent
        .spawn((marker, FloatingWindowRaisedButton, Button, node, bg, border))
        .with_children(|button| {
            button.spawn((Text::new(label), panel_body_font(), TextColor(TEXT_PRIMARY)));
        });
}

/// Raised action button with armed styling (e.g. selected production operation).
pub fn spawn_floating_raised_button_armed<M: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    marker: M,
    label: &str,
    node: Node,
) {
    parent
        .spawn((
            marker,
            Button,
            node,
            BackgroundColor(hud_raised_face_color(HudButtonShellState::Armed)),
            hud_raised_bevel_border(HudButtonShellState::Armed),
        ))
        .with_children(|button| {
            button.spawn((Text::new(label), panel_body_font(), TextColor(TEXT_PRIMARY)));
        });
}

/// Recessed subsection well for grouped content.
pub fn spawn_floating_section_well(
    parent: &mut ChildSpawnerCommands<'_>,
    content: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(WINDOW_SECTION_PADDING_PX)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(WINDOW_SECTION_BG),
            hud_recessed_bevel_border(),
        ))
        .with_children(content);
}

/// Thin separator between logical sections or matrix rows.
pub fn floating_row_separator_border() -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            border: UiRect::bottom(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor {
            left: Color::NONE,
            top: Color::NONE,
            right: Color::NONE,
            bottom: WINDOW_ROW_SEPARATOR,
        },
    )
}

/// Keep raised floating-window controls in sync on hover/press.
pub fn update_floating_window_raised_button_hover(
    mut query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<FloatingWindowRaisedButton>),
    >,
) {
    for (interaction, mut bg, mut border) in &mut query {
        let (next_bg, next_border) = hud_button_shell_style(interaction, true, false);
        *bg = next_bg;
        *border = next_border;
    }
}
