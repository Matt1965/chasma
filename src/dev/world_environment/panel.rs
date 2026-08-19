//! World environment panel spawn (Slice 11).

use bevy::prelude::*;

use super::fields::{EnvFieldId, EnvSection, fields_for_section};
use super::state::{
    DevWorldCycleToggle, DevWorldEnvironmentAction, DevWorldEnvironmentConfirmationBar,
    DevWorldEnvironmentDirtyBadge, DevWorldEnvironmentLoadStatusText, DevWorldEnvironmentSection,
    DevWorldEnvironmentStatusText, DevWorldEnvironmentValidationText, DevWorldPauseToggle,
    DevWorldTimePresetButton, DevWorldWaterEnabledToggle, DevWorldWaterSection,
};
use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipContent;
use crate::dev::widgets::{
    DevBadgeKind, DevCollapsibleSectionId, DevStatusSeverity, DevWidgetConfirmationBar,
    DevWidgetConfirmationPrompt, spawn_action_button, spawn_badge, spawn_bounded_slider_row,
    spawn_collapsible_section, spawn_confirmation_bar, spawn_status_line, spawn_toggle_row,
};

/// Spawn order: Water follows the time/cycle block and precedes lighting sections.
pub const WORLD_WATER_SECTION_ORDER: usize = 1;

pub fn spawn_environment_controls(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevWorldEnvironmentSection,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                DevWorldEnvironmentStatusText,
                DevPanelUi,
                Text::new(""),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
            ));
            spawn_status_row(root);
            spawn_toggle_row(
                root,
                "Cycle enabled",
                DevTooltipContent::new(
                    "When enabled, simulated time advances and drives environment lighting. When \
                     disabled, manual lighting values own directional and ambient output. \
                     Saved in Project Defaults.",
                ),
                DevWorldCycleToggle,
            );
            spawn_time_controls(root);
            spawn_water_section(root);
            spawn_day_section(root);
            spawn_night_section(root);
            spawn_twilight_section(root);
            spawn_manual_section(root);
            spawn_project_defaults_section(root);
        });
}

fn spawn_status_row(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
        ))
        .with_children(|row| {
            spawn_badge(
                row,
                DevBadgeKind::Dirty,
                DevTooltipContent::new(
                    "Current authored values differ from the loaded Project Defaults baseline. \
                     Unsaved changes persist while the World window is closed and across F12 toggles. \
                     They are lost on application exit unless saved.",
                ),
            );
            row.spawn((
                DevWorldEnvironmentDirtyBadge,
                DevPanelUi,
                Text::new(""),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.75, 0.35, 1.0)),
            ));
            row.spawn((
                DevWorldEnvironmentLoadStatusText,
                DevPanelUi,
                Text::new(""),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(Color::srgba(0.65, 0.78, 0.88, 1.0)),
            ));
            row.spawn((
                DevWorldEnvironmentValidationText,
                DevPanelUi,
                Text::new(""),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(Color::srgba(0.95, 0.45, 0.4, 1.0)),
            ));
        });
}

fn spawn_field_sliders(parent: &mut ChildSpawnerCommands<'_>, section: EnvSection) {
    for field in fields_for_section(section) {
        let spec = field.spec();
        spawn_bounded_slider_row(parent, spec.label, field.as_u32(), 88.0, spec.tooltip);
    }
}

fn spawn_time_controls(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|body| {
            spawn_toggle_row(
                body,
                "Paused",
                DevTooltipContent::new(
                    "Stops advancing the visual clock while the cycle remains enabled. Runtime \
                     only — not saved in Project Defaults.",
                ),
                DevWorldPauseToggle,
            );
            spawn_field_sliders(body, EnvSection::TimeCycle);
            body.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
            ))
            .with_children(|row| {
                for (label, preset) in [
                    ("Dawn", super::state::WorldTimePreset::Dawn),
                    ("Noon", super::state::WorldTimePreset::Noon),
                    ("Night", super::state::WorldTimePreset::Midnight),
                ] {
                    spawn_action_button(
                        row,
                        label,
                        Some("Jump the runtime clock to this preset hour. Not saved in Project Defaults."),
                        DevWorldTimePresetButton { preset },
                    );
                }
            });
        });
}

fn spawn_day_section(parent: &mut ChildSpawnerCommands<'_>) {
    spawn_collapsible_section(
        parent,
        DevCollapsibleSectionId::WorldDayLighting,
        "Day lighting",
        Some(DevTooltipContent::new(
            "Noon peak directional, ambient, and sun elevation. Values blend with night settings \
             through twilight.",
        )),
        |body| spawn_field_sliders(body, EnvSection::DayLighting),
    );
}

fn spawn_night_section(parent: &mut ChildSpawnerCommands<'_>) {
    spawn_collapsible_section(
        parent,
        DevCollapsibleSectionId::WorldNightLighting,
        "Night lighting",
        Some(DevTooltipContent::new(
            "Deep-night directional values plus the ambient multiplier applied to noon ambient at \
             night.",
        )),
        |body| spawn_field_sliders(body, EnvSection::NightLighting),
    );
}

fn spawn_twilight_section(parent: &mut ChildSpawnerCommands<'_>) {
    spawn_collapsible_section(
        parent,
        DevCollapsibleSectionId::WorldTwilight,
        "Twilight",
        Some(DevTooltipContent::new(
            "Sunrise/sunset thresholds and twilight warmth blend. Sunrise must be earlier than \
             sunset.",
        )),
        |body| spawn_field_sliders(body, EnvSection::Twilight),
    );
}

fn spawn_manual_section(parent: &mut ChildSpawnerCommands<'_>) {
    spawn_collapsible_section(
        parent,
        DevCollapsibleSectionId::WorldManualLighting,
        "Manual lighting",
        Some(DevTooltipContent::new(
            "Fixed lighting used when the visual cycle is disabled. While the cycle runs, these \
             values are stored but overridden by time-of-day evaluation.",
        )),
        |body| spawn_field_sliders(body, EnvSection::ManualLighting),
    );
}

fn spawn_water_section(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevWorldWaterSection,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|water_root| {
            spawn_collapsible_section(
                water_root,
                DevCollapsibleSectionId::WorldWater,
                "Water",
                Some(DevTooltipContent::new(
                    "Environment ocean plane controls. Adjust level to align the water surface with \
                     terrain. Does not affect terrain hydrology or the gameplay water field.",
                )),
                |body| {
                    spawn_toggle_row(
                        body,
                        "Water enabled",
                        DevTooltipContent::new(
                            "Show or hide the environment ocean plane (EnvironmentWaterPlane).",
                        ),
                        DevWorldWaterEnabledToggle,
                    );
                    spawn_bounded_slider_row(
                        body,
                        "Water Level",
                        super::water::WORLD_WATER_LEVEL_FIELD_ID,
                        88.0,
                        "World-space height of the environment water plane.",
                    );
                },
            );
        });
}

fn spawn_project_defaults_section(parent: &mut ChildSpawnerCommands<'_>) {
    spawn_collapsible_section(
        parent,
        DevCollapsibleSectionId::WorldProjectDefaults,
        "Project defaults",
        Some(DevTooltipContent::new(
            "Load, save, and revert authored environment baselines in \
             assets/environment/project_defaults.ron. Scene saves remain independent.",
        )),
        |body| {
            body.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
            ))
            .with_children(|row| {
                spawn_action_button(
                    row,
                    "Save as Project Defaults",
                    Some(
                        "Writes the current authored values to assets/environment/project_defaults.ron. \
                         Released builds load these values on startup. Scene files are not modified.",
                    ),
                    DevWorldEnvironmentAction::SaveProjectDefaults,
                );
                spawn_action_button(
                    row,
                    "Revert",
                    Some(
                        "Restores the last successfully loaded or saved Project Defaults. Does not \
                         restore Chasma built-in fallback values — use Reset to Built-in Defaults for that.",
                    ),
                    DevWorldEnvironmentAction::Revert,
                );
                spawn_action_button(
                    row,
                    "Reset to Built-in Defaults",
                    Some(
                        "Loads immutable built-in baseline values into the active environment without \
                         writing the project file. Marks dirty if built-in differs from project defaults.",
                    ),
                    DevWorldEnvironmentAction::ResetBuiltIn,
                );
            });
            spawn_confirmation_bar(
                body,
                "",
                "Confirm",
                "Cancel",
                DevWorldEnvironmentAction::Confirm,
                DevWorldEnvironmentAction::CancelConfirm,
                DevTooltipContent::new("Confirm or cancel the pending project-defaults action."),
                DevWorldEnvironmentConfirmationBar,
            );
            spawn_status_line(body, DevStatusSeverity::Info, "");
        },
    );
}
