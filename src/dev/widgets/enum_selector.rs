//! Segmented enum selector for small option sets (Slice 9).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipContent;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::window::DevWindowUi;

use super::theme::{TEXT_PRIMARY, action_button_bg, small_text_font};

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWidgetSegmentedOption {
    pub index: usize,
}

#[derive(Component, Debug)]
pub struct DevWidgetSegmentedControl {
    pub selected: usize,
}

/// Spawn a horizontal segmented control; domain handler reads `index` on press.
pub fn spawn_segmented_control(
    parent: &mut ChildSpawnerCommands<'_>,
    options: &[(&str, DevTooltipContent)],
    selected: usize,
) {
    parent
        .spawn((
            DevWidgetSegmentedControl { selected },
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
        ))
        .with_children(|row| {
            for (index, (label, tip)) in options.iter().enumerate() {
                row.spawn((
                    DevWidgetSegmentedOption { index },
                    DevTooltipTarget::from_content(tip.clone()),
                    DevPanelUi,
                    DevWindowUi,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(super::theme::BTN_BG_IDLE),
                    Text::new(*label),
                    small_text_font(),
                    TextColor(TEXT_PRIMARY),
                ));
            }
        });
}

pub fn sync_segmented_styles(
    selected: usize,
    mut buttons: Query<
        (
            &Interaction,
            &DevWidgetSegmentedOption,
            &mut BackgroundColor,
        ),
        With<Button>,
    >,
) {
    for (interaction, option, mut bg) in &mut buttons {
        *bg = action_button_bg(interaction, option.index == selected);
    }
}
