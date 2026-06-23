//! Minimal SC2-style HUD — selection count, command state, portrait placeholders.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::state::{command_state_display, GameplayCommandState, GameplayUiState};

/// Root gameplay HUD panel (screen-space, bottom-left).
#[derive(Component, Debug)]
pub struct GameplayHudRoot;

#[derive(Component, Debug)]
pub(crate) struct HudSelectionCountText;

#[derive(Component, Debug)]
pub(crate) struct HudCommandStateText;

#[derive(Component, Debug)]
pub(crate) struct HudDebugBadge;

#[derive(Component, Debug)]
pub(crate) struct HudPortraitRow;

#[derive(Component, Debug)]
pub(crate) struct HudPortraitSlot {
    unit_slot: u8,
}

const MAX_PORTRAIT_SLOTS: u8 = 8;
const PORTRAIT_SIZE_PX: f32 = 36.0;

/// Spawn the gameplay HUD tree once at startup.
pub fn setup_gameplay_hud(mut commands: Commands) {
    commands
        .spawn((
            GameplayHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                bottom: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.1, 0.72)),
            FocusPolicy::Pass,
            ZIndex(300),
        ))
        .with_children(|panel| {
            panel.spawn((
                HudSelectionCountText,
                Text::new("Selected: 0"),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.95, 1.0, 1.0)),
            ));
            panel.spawn((
                HudCommandStateText,
                Text::new("Command: Idle"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.85, 0.95, 1.0)),
            ));
            panel
                .spawn((
                    HudPortraitRow,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    for slot in 0..MAX_PORTRAIT_SLOTS {
                        row.spawn((
                            HudPortraitSlot { unit_slot: slot },
                            Node {
                                width: Val::Px(PORTRAIT_SIZE_PX),
                                height: Val::Px(PORTRAIT_SIZE_PX),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.2, 0.35, 0.45, 0.85)),
                            BorderColor::all(Color::srgba(0.35, 0.55, 0.65, 0.9)),
                        ));
                    }
                });
            panel.spawn((
                HudDebugBadge,
                Text::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 0.75, 0.25, 0.9)),
                Node {
                    display: Display::None,
                    ..default()
                },
            ));
        });
}

/// Refresh HUD text and portrait highlights only when gameplay UI state changes.
pub fn sync_gameplay_hud(
    ui_state: Res<GameplayUiState>,
    mut count_text: Query<
        &mut Text,
        (With<HudSelectionCountText>, Without<HudCommandStateText>),
    >,
    mut command_text: Query<
        &mut Text,
        (
            With<HudCommandStateText>,
            Without<HudSelectionCountText>,
            Without<HudDebugBadge>,
        ),
    >,
    mut debug_badge: Query<
        (&mut Text, &mut Node),
        (
            With<HudDebugBadge>,
            Without<HudSelectionCountText>,
            Without<HudCommandStateText>,
        ),
    >,
    mut portraits: Query<
        (&HudPortraitSlot, &mut BackgroundColor, &mut BorderColor),
        Without<HudSelectionCountText>,
    >,
) {
    if !ui_state.hud_dirty {
        return;
    }

    if let Ok(mut text) = count_text.single_mut() {
        **text = format!("Selected: {}", ui_state.snapshot.selection_count);
    }

    if let Ok(mut text) = command_text.single_mut() {
        let label = command_state_display(
            ui_state.snapshot.command_state,
            ui_state.snapshot.resolved_command_type,
        );
        let tooltip = ui_state
            .snapshot
            .command_tooltip
            .as_deref()
            .unwrap_or("");
        **text = if tooltip.is_empty() {
            format!("Command: {label}")
        } else {
            format!("Command: {label} — {tooltip}")
        };
    }

    if let Ok((mut text, mut node)) = debug_badge.single_mut() {
        if ui_state.snapshot.debug_overlay_active {
            **text = "DBG".to_string();
            node.display = Display::Flex;
        } else {
            **text = String::new();
            node.display = Display::None;
        }
    }

    let leader_slot = ui_state
        .snapshot
        .leader_unit
        .map(|_| 0_u8)
        .unwrap_or(u8::MAX);
    let visible = ui_state.snapshot.selection_count.min(MAX_PORTRAIT_SLOTS as u32) as u8;

    for (slot, mut fill, mut border) in &mut portraits {
        if slot.unit_slot >= visible {
            fill.0 = Color::srgba(0.12, 0.18, 0.22, 0.35);
            border.set_all(Color::srgba(0.25, 0.3, 0.35, 0.5));
            continue;
        }
        let is_leader = slot.unit_slot == leader_slot && visible > 0;
        fill.0 = if is_leader {
            Color::srgba(0.25, 0.55, 0.35, 0.95)
        } else {
            Color::srgba(0.2, 0.35, 0.45, 0.85)
        };
        border.set_all(if is_leader {
            Color::srgba(0.45, 0.95, 0.35, 1.0)
        } else {
            Color::srgba(0.35, 0.55, 0.65, 0.9)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::{
        command_state_display, derive_gameplay_snapshot, GameplayCommandState, GameplayCursorMode,
    };
    use crate::client::ResolvedCommandFeedback;
    use crate::units::input::SelectedUnits;
    use crate::debug::{CommandTraceBuffer, IntentDispatchHistory};
    use crate::world::UnitId;

    #[test]
    fn selection_ui_count_matches_selected_units() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([UnitId::new(1), UnitId::new(2)]);
        let snapshot = derive_gameplay_snapshot(
            &selection,
            &IntentDispatchHistory::default(),
            &CommandTraceBuffer::default(),
            &ResolvedCommandFeedback::default(),
            false,
            GameplayCursorMode::Default,
        );
        assert_eq!(snapshot.selection_count, 2);
    }

    #[test]
    fn command_state_label_for_moving() {
        assert_eq!(
            command_state_display(GameplayCommandState::Moving, None),
            "Move"
        );
    }
}
