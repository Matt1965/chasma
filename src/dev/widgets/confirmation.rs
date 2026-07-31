//! Destructive-action confirmation bar helpers (Slice 9).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipContent;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::window::DevWindowUi;

use super::button::spawn_action_button;
use super::theme::{STATUS_WARNING, small_text_font};

#[derive(Component, Debug)]
pub struct DevWidgetConfirmationBar;

#[derive(Component, Debug)]
pub struct DevWidgetConfirmationPrompt;

/// Spawn confirm/cancel row; domain panels control visibility via existing state.
pub fn spawn_confirmation_bar<Confirm: Component, Cancel: Component, Extra: Component + Clone>(
    parent: &mut ChildSpawnerCommands<'_>,
    prompt: &str,
    confirm_label: &str,
    cancel_label: &str,
    confirm_marker: Confirm,
    cancel_marker: Cancel,
    tooltip: DevTooltipContent,
    extra: Extra,
) {
    let prompt_marker = extra.clone();
    parent
        .spawn((
            DevWidgetConfirmationBar,
            extra,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|bar| {
            bar.spawn((
                DevWidgetConfirmationPrompt,
                prompt_marker,
                DevTooltipTarget::from_content(tooltip),
                DevPanelUi,
                Text::new(prompt),
                small_text_font(),
                TextColor(STATUS_WARNING),
            ));
            bar.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|row| {
                spawn_action_button(row, confirm_label, None, confirm_marker);
                spawn_action_button(row, cancel_label, None, cancel_marker);
            });
        });
}

pub fn set_confirmation_visible(bar: &mut Node, visible: bool) {
    bar.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
}
