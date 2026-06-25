//! Dev panel controls for environment lighting tuning (World tab).

use bevy::prelude::*;

use crate::environment::{EnvironmentSettings, TimeOfDaySettings};

use super::dev_mode::{DevModeState, DevTab};
use super::input::DevPanelUi;
use crate::dev::DevModeInputGate;

#[derive(Component, Debug)]
pub(crate) struct DevLightingSection;

#[derive(Component, Debug)]
pub(crate) struct DevLightingStatusText;

#[derive(Debug, Clone, Copy)]
pub(crate) enum LightingTuneField {
    NoonDirectional,
    NightDirectional,
    NoonAmbient,
    NightAmbientMult,
    NoonSkybox,
    NightSkybox,
    TwilightBlend,
    SunPitchMin,
    SunPitchMax,
    SunriseHour,
    SunsetHour,
    ManualDirectional,
    ManualAmbient,
    ManualSkybox,
}

#[derive(Component, Debug)]
pub(crate) struct DevLightingTuneButton {
    pub field: LightingTuneField,
    pub delta: f32,
}

pub(crate) fn spawn_lighting_section(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevLightingSection,
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
                DevLightingStatusText,
                DevPanelUi,
                Text::new("Lighting"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.8, 0.85, 0.92, 1.0)),
            ));
            spawn_tune_row(section, "Noon dir", LightingTuneField::NoonDirectional, 1_000.0);
            spawn_tune_row(section, "Night dir", LightingTuneField::NightDirectional, 10.0);
            spawn_tune_row(section, "Noon amb", LightingTuneField::NoonAmbient, 20.0);
            spawn_tune_row(section, "Night amb x", LightingTuneField::NightAmbientMult, 0.05);
            spawn_tune_row(section, "Noon sky", LightingTuneField::NoonSkybox, 50.0);
            spawn_tune_row(section, "Night sky", LightingTuneField::NightSkybox, 10.0);
            spawn_tune_row(section, "Twilight", LightingTuneField::TwilightBlend, 0.05);
            spawn_tune_row(section, "Sun min", LightingTuneField::SunPitchMin, 2.0);
            spawn_tune_row(section, "Sun max", LightingTuneField::SunPitchMax, 2.0);
            spawn_tune_row(section, "Sunrise", LightingTuneField::SunriseHour, 0.5);
            spawn_tune_row(section, "Sunset", LightingTuneField::SunsetHour, 0.5);
            spawn_tune_row(section, "Manual dir", LightingTuneField::ManualDirectional, 500.0);
            spawn_tune_row(section, "Manual amb", LightingTuneField::ManualAmbient, 20.0);
            spawn_tune_row(section, "Manual sky", LightingTuneField::ManualSkybox, 50.0);
        });
}

fn spawn_tune_row(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    field: LightingTuneField,
    step: f32,
) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                DevPanelUi,
                Text::new(label),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgba(0.7, 0.78, 0.86, 1.0)),
                Node {
                    width: Val::Px(72.0),
                    ..default()
                },
            ));
            spawn_tune_button(row, field, -step);
            spawn_tune_button(row, field, step);
        });
}

fn spawn_tune_button(parent: &mut ChildSpawnerCommands<'_>, field: LightingTuneField, delta: f32) {
    let label = if delta < 0.0 { "-" } else { "+" };
    parent.spawn((
        DevLightingTuneButton { field, delta },
        DevPanelUi,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.2, 0.28, 0.95)),
        Text::new(label),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
    ));
}

pub(crate) fn sync_lighting_section_visibility(
    dev_state: Res<DevModeState>,
    mut section: Query<&mut Node, With<DevLightingSection>>,
) {
    if !dev_state.enabled {
        return;
    }
    let show = dev_state.active_tab == DevTab::WorldTools;
    if let Ok(mut node) = section.single_mut() {
        node.display = if show { Display::Flex } else { Display::None };
    }
}

pub(crate) fn sync_lighting_panel_text(
    dev_state: Res<DevModeState>,
    time_of_day: Res<TimeOfDaySettings>,
    environment: Res<EnvironmentSettings>,
    mut text: Query<&mut Text, With<DevLightingStatusText>>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::WorldTools {
        return;
    }
    let Ok(mut label) = text.single_mut() else {
        return;
    };
    **label = format!(
        "Lighting (cycle={})\n\
         noon dir={:.0} night dir={:.0}\n\
         noon amb={:.0} night amb x={:.2}\n\
         noon sky={:.0} night sky={:.0} twilight={:.2}\n\
         sun pitch {:.0}/{:.0} rise/set {:.1}/{:.1}\n\
         manual dir={:.0} amb={:.0} sky={:.0}",
        if time_of_day.enabled { "on" } else { "off" },
        time_of_day.noon_directional_illuminance,
        time_of_day.night_directional_illuminance,
        time_of_day.noon_ambient_brightness,
        time_of_day.night_ambient_multiplier,
        time_of_day.noon_skybox_brightness,
        time_of_day.night_skybox_brightness,
        time_of_day.twilight_daylight_blend,
        time_of_day.sun_pitch_min_deg,
        time_of_day.sun_pitch_max_deg,
        time_of_day.sunrise_hour,
        time_of_day.sunset_hour,
        environment.directional_light_illuminance,
        environment.ambient_brightness,
        environment.skybox_brightness,
    );
}

pub(crate) fn handle_lighting_tune_buttons(
    dev_state: Res<DevModeState>,
    mut gate: ResMut<DevModeInputGate>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
    mut environment: ResMut<EnvironmentSettings>,
    buttons: Query<(&Interaction, &DevLightingTuneButton), Changed<Interaction>>,
) {
    if !dev_state.enabled {
        return;
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        apply_lighting_tune(
            button.field,
            button.delta,
            &mut time_of_day,
            &mut environment,
        );
    }
}

fn apply_lighting_tune(
    field: LightingTuneField,
    delta: f32,
    time_of_day: &mut TimeOfDaySettings,
    environment: &mut EnvironmentSettings,
) {
    match field {
        LightingTuneField::NoonDirectional => {
            time_of_day.noon_directional_illuminance =
                (time_of_day.noon_directional_illuminance + delta).max(0.0);
        }
        LightingTuneField::NightDirectional => {
            time_of_day.night_directional_illuminance =
                (time_of_day.night_directional_illuminance + delta).max(0.0);
        }
        LightingTuneField::NoonAmbient => {
            time_of_day.noon_ambient_brightness =
                (time_of_day.noon_ambient_brightness + delta).max(0.0);
        }
        LightingTuneField::NightAmbientMult => {
            time_of_day.night_ambient_multiplier =
                (time_of_day.night_ambient_multiplier + delta).clamp(0.0, 2.0);
        }
        LightingTuneField::NoonSkybox => {
            time_of_day.noon_skybox_brightness =
                (time_of_day.noon_skybox_brightness + delta).max(0.0);
        }
        LightingTuneField::NightSkybox => {
            time_of_day.night_skybox_brightness =
                (time_of_day.night_skybox_brightness + delta).max(0.0);
        }
        LightingTuneField::TwilightBlend => {
            time_of_day.twilight_daylight_blend =
                (time_of_day.twilight_daylight_blend + delta).clamp(0.0, 1.0);
        }
        LightingTuneField::SunPitchMin => {
            time_of_day.sun_pitch_min_deg = (time_of_day.sun_pitch_min_deg + delta).clamp(-90.0, 90.0);
        }
        LightingTuneField::SunPitchMax => {
            time_of_day.sun_pitch_max_deg = (time_of_day.sun_pitch_max_deg + delta).clamp(-90.0, 90.0);
        }
        LightingTuneField::SunriseHour => {
            time_of_day.sunrise_hour = (time_of_day.sunrise_hour + delta).clamp(0.0, 23.0);
        }
        LightingTuneField::SunsetHour => {
            time_of_day.sunset_hour = (time_of_day.sunset_hour + delta).clamp(0.0, 24.0);
        }
        LightingTuneField::ManualDirectional => {
            environment.directional_light_illuminance =
                (environment.directional_light_illuminance + delta).max(0.0);
        }
        LightingTuneField::ManualAmbient => {
            environment.ambient_brightness = (environment.ambient_brightness + delta).max(0.0);
        }
        LightingTuneField::ManualSkybox => {
            environment.skybox_brightness = (environment.skybox_brightness + delta).max(0.0);
        }
    }
}
