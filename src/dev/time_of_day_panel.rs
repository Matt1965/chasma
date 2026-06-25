//! Dev panel controls for visual time of day (World tab).

use bevy::prelude::*;

use crate::environment::{apply_time_of_day_dev_action, format_time_of_day_status, TimeOfDayDevAction, TimeOfDaySettings};

use super::dev_mode::{DevModeState, DevTab};
use super::input::DevPanelUi;
use crate::dev::DevModeInputGate;

#[derive(Component, Debug)]
pub(crate) struct DevTimeOfDaySection;

#[derive(Component, Debug)]
pub(crate) struct DevTimeOfDayText;

#[derive(Component, Debug)]
pub(crate) struct DevTimeOfDayButton {
    pub action: TimeOfDayDevAction,
}

pub(crate) fn spawn_time_of_day_section(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevTimeOfDaySection,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|section| {
            section.spawn((
                DevTimeOfDayText,
                DevPanelUi,
                Text::new("Time of day"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.8, 0.85, 0.92, 1.0)),
            ));
            spawn_time_button_row(section, &[("-1h", TimeOfDayDevAction::HourEarlier), ("+1h", TimeOfDayDevAction::HourLater)]);
            spawn_time_button_row(
                section,
                &[
                    ("Dawn", TimeOfDayDevAction::SetDawn),
                    ("Noon", TimeOfDayDevAction::SetNoon),
                    ("Night", TimeOfDayDevAction::SetMidnight),
                ],
            );
            spawn_time_button_row(
                section,
                &[
                    ("Cycle", TimeOfDayDevAction::ToggleEnabled),
                    ("Pause", TimeOfDayDevAction::TogglePaused),
                    ("Slower", TimeOfDayDevAction::SlowerDay),
                    ("Faster", TimeOfDayDevAction::FasterDay),
                ],
            );
        });
}

fn spawn_time_button_row(parent: &mut ChildSpawnerCommands<'_>, buttons: &[(&str, TimeOfDayDevAction)]) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
        ))
        .with_children(|row| {
            for (label, action) in buttons {
                row.spawn((
                    DevTimeOfDayButton { action: *action },
                    DevPanelUi,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.2, 0.28, 0.95)),
                    Text::new(*label),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
                ));
            }
        });
}

pub(crate) fn sync_time_of_day_section_visibility(
    dev_state: Res<DevModeState>,
    mut section: Query<&mut Node, With<DevTimeOfDaySection>>,
) {
    if !dev_state.enabled {
        return;
    }
    let show = dev_state.active_tab == DevTab::WorldTools;
    if let Ok(mut node) = section.single_mut() {
        node.display = if show { Display::Flex } else { Display::None };
    }
}

pub(crate) fn sync_time_of_day_panel_text(
    dev_state: Res<DevModeState>,
    time_of_day: Res<TimeOfDaySettings>,
    mut text: Query<&mut Text, With<DevTimeOfDayText>>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::WorldTools {
        return;
    }
    let Ok(mut label) = text.single_mut() else {
        return;
    };
    **label = format_time_of_day_status(&time_of_day);
}

pub(crate) fn handle_time_of_day_buttons(
    dev_state: Res<DevModeState>,
    mut gate: ResMut<DevModeInputGate>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
    buttons: Query<(&Interaction, &DevTimeOfDayButton), Changed<Interaction>>,
) {
    if !dev_state.enabled {
        return;
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        apply_time_of_day_dev_action(button.action, &mut time_of_day);
    }
}
