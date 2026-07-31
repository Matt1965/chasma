//! World environment UI systems (Slice 11).

use bevy::ecs::system::ParamSet;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::dev::dev_mode::{DevModeState, DevTextFieldFocus};
use crate::dev::input::DevPanelUi;
use crate::dev::widgets::{
    DevSliderDragState, DevWidgetConfirmationBar, DevWidgetConfirmationPrompt,
    DevWidgetSliderTrack, DevWidgetSliderValue, DevWidgetToggle, NumericParseResult,
    apply_numeric_bounds, format_numeric_display, normalized_to_value, parse_numeric_draft,
    set_confirmation_visible, slider_normalized_x, sync_slider_fill, value_to_normalized,
};
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::environment::{
    EnvironmentManualLighting, EnvironmentSettings, ProjectDefaultsLoadStatus,
    ProjectEnvironmentBaseline, TimeOfDayDevAction, TimeOfDaySettings,
    apply_time_of_day_dev_action, built_in_authored_snapshot, capture_current_authored_snapshot,
    environment_is_dirty, format_time_of_day_status, list_registered_skybox_sets,
    save_project_environment_defaults, validate_authored_snapshot,
};

use super::fields::EnvFieldId;
use super::state::{
    DevWorldCycleToggle, DevWorldEnvironmentAction, DevWorldEnvironmentConfirmationBar,
    DevWorldEnvironmentDirtyBadge, DevWorldEnvironmentLoadStatusText, DevWorldEnvironmentSection,
    DevWorldEnvironmentStatusText, DevWorldEnvironmentValidationText, DevWorldPauseToggle,
    DevWorldSkyboxOption, DevWorldTimePresetButton, WorldEnvironmentConfirm,
    WorldEnvironmentUiState,
};

pub fn sync_world_environment_panel(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    baseline: Res<ProjectEnvironmentBaseline>,
    time_of_day: Res<TimeOfDaySettings>,
    environment: Res<EnvironmentSettings>,
    manual: Res<EnvironmentManualLighting>,
    ui_state: Res<WorldEnvironmentUiState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<DevWorldEnvironmentStatusText>>,
        Query<&mut Text, With<DevWorldEnvironmentDirtyBadge>>,
        Query<&mut Text, With<DevWorldEnvironmentLoadStatusText>>,
        Query<&mut Text, With<DevWorldEnvironmentValidationText>>,
    )>,
    mut dirty_badges: Query<&mut Visibility, With<crate::dev::widgets::DevWidgetBadge>>,
) {
    let active = registry.window_active(dev_state.enabled, DevWindowId::World);
    if !active {
        for mut vis in &mut dirty_badges {
            *vis = Visibility::Hidden;
        }
        return;
    }

    if let Ok(mut text) = texts.p0().single_mut() {
        **text = format_time_of_day_status(&time_of_day);
    }

    let dirty = environment_is_dirty(&baseline, &time_of_day, &environment, &manual);
    if let Ok(mut text) = texts.p1().single_mut() {
        **text = if dirty { "Unsaved changes" } else { "" }.into();
    }
    for mut vis in &mut dirty_badges {
        *vis = if dirty {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if let Ok(mut text) = texts.p2().single_mut() {
        let path = baseline.source_path.display();
        **text = match &baseline.load_status {
            ProjectDefaultsLoadStatus::LoadedFromFile => {
                format!("Baseline: {path}")
            }
            ProjectDefaultsLoadStatus::MissingFileUsedBuiltIn => {
                format!("Baseline: built-in (no {path})")
            }
            ProjectDefaultsLoadStatus::InvalidFileUsedBuiltIn { error } => {
                format!("Baseline: built-in fallback ({error})")
            }
            ProjectDefaultsLoadStatus::NotLoaded => "Baseline: pending".into(),
        };
    }

    if let Ok(mut text) = texts.p3().single_mut() {
        **text = ui_state
            .validation_error
            .as_ref()
            .map(|e| e.message())
            .unwrap_or_default();
    }
}

pub fn sync_world_environment_sliders(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    time_of_day: Res<TimeOfDaySettings>,
    environment: Res<EnvironmentSettings>,
    manual: Res<EnvironmentManualLighting>,
    ui_state: Res<WorldEnvironmentUiState>,
    mut tracks: Query<(&DevWidgetSliderTrack, &Children)>,
    mut fills: Query<&mut Node, Without<DevWidgetSliderTrack>>,
    mut values: Query<(&DevWidgetSliderValue, &mut Text)>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }

    for field in EnvFieldId::ALL {
        let spec = field.spec();
        let authoritative = field.read(&time_of_day, &environment, &manual);
        let display = if ui_state.focused_field == Some(field) {
            ui_state
                .numeric_drafts
                .get(&field)
                .map(|d| d.text.clone())
                .unwrap_or_else(|| format_numeric_display(authoritative, spec.precision))
        } else {
            format_numeric_display(authoritative, spec.precision)
        };
        let norm = value_to_normalized(authoritative, spec.min, spec.max);
        sync_slider_fill(
            field.as_u32(),
            norm,
            &display,
            &mut tracks,
            &mut fills,
            &mut values,
        );
    }
}

pub fn sync_world_environment_toggles(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    time_of_day: Res<TimeOfDaySettings>,
    mut toggles: ParamSet<(
        Query<(
            &DevWorldCycleToggle,
            &DevWidgetToggle,
            &mut BackgroundColor,
            &Children,
        )>,
        Query<
            (
                &DevWorldPauseToggle,
                &DevWidgetToggle,
                &mut BackgroundColor,
                &Children,
            ),
            Without<DevWorldCycleToggle>,
        >,
    )>,
    mut marks: Query<&mut Visibility, With<crate::dev::widgets::DevWidgetToggleMark>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (_marker, toggle, mut bg, children) in toggles.p0().iter_mut() {
        let on = time_of_day.enabled;
        *bg = crate::dev::widgets::toggle_button_bg(&Interaction::None, on, toggle.disabled);
        sync_toggle_mark(children, on && !toggle.disabled, &mut marks);
    }
    for (_marker, toggle, mut bg, children) in toggles.p1().iter_mut() {
        let on = time_of_day.paused;
        *bg = crate::dev::widgets::toggle_button_bg(&Interaction::None, on, toggle.disabled);
        sync_toggle_mark(children, on && !toggle.disabled, &mut marks);
    }
}

fn sync_toggle_mark(
    children: &Children,
    show: bool,
    marks: &mut Query<&mut Visibility, With<crate::dev::widgets::DevWidgetToggleMark>>,
) {
    for child in children.iter() {
        if let Ok(mut visibility) = marks.get_mut(child) {
            *visibility = if show {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub fn sync_world_skybox_buttons(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    environment: Res<EnvironmentSettings>,
    mut buttons: Query<(&DevWorldSkyboxOption, &Interaction, &mut BackgroundColor), With<Button>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    let sets = list_registered_skybox_sets();
    for (option, interaction, mut bg) in &mut buttons {
        let is_active = sets
            .get(option.index)
            .map(|s| s == &environment.skybox_set)
            .unwrap_or(false);
        *bg = crate::dev::widgets::action_button_bg(interaction, is_active);
    }
}

pub fn sync_world_environment_confirm_bar(
    ui_state: Res<WorldEnvironmentUiState>,
    mut bars: Query<
        (&DevWidgetConfirmationBar, &mut Node),
        With<DevWorldEnvironmentConfirmationBar>,
    >,
    mut prompts: Query<
        &mut Text,
        (
            With<DevWidgetConfirmationPrompt>,
            With<DevWorldEnvironmentConfirmationBar>,
        ),
    >,
) {
    let visible = ui_state.pending_confirmation.is_some();
    for (_bar, mut node) in &mut bars {
        set_confirmation_visible(&mut node, visible);
    }
    if let Some(action) = ui_state.pending_confirmation {
        let prompt = match action {
            WorldEnvironmentConfirm::Revert => "Revert to the last loaded/saved Project Defaults?",
            WorldEnvironmentConfirm::ResetBuiltIn => {
                "Reset active environment to Chasma built-in defaults (does not write the project file)?"
            }
            WorldEnvironmentConfirm::SaveProjectDefaults => {
                "Save current authored values to assets/environment/project_defaults.ron? Released builds will load these values. Scene saves are not modified."
            }
        };
        if let Ok(mut text) = prompts.single_mut() {
            **text = prompt.into();
        }
    }
}

pub fn tick_world_environment_status(mut ui_state: ResMut<WorldEnvironmentUiState>) {
    ui_state.tick_status();
}

pub fn handle_world_environment_actions(
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    mut ui_state: ResMut<WorldEnvironmentUiState>,
    mut baseline: ResMut<ProjectEnvironmentBaseline>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
    mut environment: ResMut<EnvironmentSettings>,
    mut manual: ResMut<EnvironmentManualLighting>,
    buttons: Query<(&Interaction, &DevWorldEnvironmentAction), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        match action {
            DevWorldEnvironmentAction::SaveProjectDefaults => {
                ui_state.pending_confirmation = Some(WorldEnvironmentConfirm::SaveProjectDefaults);
            }
            DevWorldEnvironmentAction::Revert => {
                if environment_is_dirty(&baseline, &time_of_day, &environment, &manual) {
                    ui_state.pending_confirmation = Some(WorldEnvironmentConfirm::Revert);
                } else {
                    apply_revert(
                        &baseline,
                        &mut time_of_day,
                        &mut environment,
                        &mut manual,
                        &mut ui_state,
                    );
                }
            }
            DevWorldEnvironmentAction::ResetBuiltIn => {
                ui_state.pending_confirmation = Some(WorldEnvironmentConfirm::ResetBuiltIn);
            }
            DevWorldEnvironmentAction::Confirm => {
                let pending = ui_state.pending_confirmation.take();
                match pending {
                    Some(WorldEnvironmentConfirm::SaveProjectDefaults) => {
                        perform_save(
                            &mut baseline,
                            &time_of_day,
                            &environment,
                            &manual,
                            &mut ui_state,
                        );
                    }
                    Some(WorldEnvironmentConfirm::Revert) => apply_revert(
                        &baseline,
                        &mut time_of_day,
                        &mut environment,
                        &mut manual,
                        &mut ui_state,
                    ),
                    Some(WorldEnvironmentConfirm::ResetBuiltIn) => apply_reset_built_in(
                        &mut time_of_day,
                        &mut environment,
                        &mut manual,
                        &mut ui_state,
                    ),
                    None => {}
                }
            }
            DevWorldEnvironmentAction::CancelConfirm => {
                ui_state.pending_confirmation = None;
            }
        }
    }
}

fn perform_save(
    baseline: &mut ProjectEnvironmentBaseline,
    time_of_day: &TimeOfDaySettings,
    environment: &EnvironmentSettings,
    manual: &EnvironmentManualLighting,
    ui_state: &mut WorldEnvironmentUiState,
) {
    let snapshot = capture_current_authored_snapshot(time_of_day, environment, manual);
    if let Err(err) = validate_authored_snapshot(&snapshot) {
        ui_state.validation_error = Some(err);
        ui_state.set_error("Save blocked — fix validation errors first");
        return;
    }
    match save_project_environment_defaults(&baseline.source_path, &snapshot) {
        Ok(()) => {
            baseline.snapshot = snapshot;
            baseline.load_status = ProjectDefaultsLoadStatus::LoadedFromFile;
            ui_state.validation_error = None;
            ui_state.set_success("Saved project environment defaults");
        }
        Err(err) => {
            ui_state.set_error(format!("Save failed: {err}"));
        }
    }
}

fn apply_revert(
    baseline: &ProjectEnvironmentBaseline,
    time_of_day: &mut TimeOfDaySettings,
    environment: &mut EnvironmentSettings,
    manual: &mut EnvironmentManualLighting,
    ui_state: &mut WorldEnvironmentUiState,
) {
    let hours = time_of_day.time_hours;
    let paused = time_of_day.paused;
    baseline
        .snapshot
        .apply_to_runtime(time_of_day, environment, &mut manual.values);
    time_of_day.time_hours = hours;
    time_of_day.paused = paused;
    ui_state.clear_numeric_drafts();
    ui_state.validation_error = validate_authored_snapshot(&baseline.snapshot).err();
    ui_state.set_success("Reverted to project baseline");
}

fn apply_reset_built_in(
    time_of_day: &mut TimeOfDaySettings,
    environment: &mut EnvironmentSettings,
    manual: &mut EnvironmentManualLighting,
    ui_state: &mut WorldEnvironmentUiState,
) {
    let hours = time_of_day.time_hours;
    let paused = time_of_day.paused;
    let built_in = built_in_authored_snapshot();
    built_in.apply_to_runtime(time_of_day, environment, &mut manual.values);
    time_of_day.time_hours = hours;
    time_of_day.paused = paused;
    ui_state.clear_numeric_drafts();
    ui_state.validation_error = validate_authored_snapshot(&built_in).err();
    ui_state.set_success("Reset to built-in defaults (not saved)");
}

pub fn handle_world_cycle_toggles(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
    mut environment: ResMut<EnvironmentSettings>,
    mut manual: ResMut<EnvironmentManualLighting>,
    mut ui_state: ResMut<WorldEnvironmentUiState>,
    buttons: Query<
        (
            &Interaction,
            &DevWidgetToggle,
            Option<&DevWorldCycleToggle>,
            Option<&DevWorldPauseToggle>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (interaction, _toggle, cycle, pause) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        if cycle.is_some() {
            apply_time_of_day_dev_action(TimeOfDayDevAction::ToggleEnabled, &mut time_of_day);
            if !time_of_day.enabled {
                crate::environment::apply_manual_lighting(&mut environment, &manual.values);
            }
            ui_state.validation_error = validate_authored_snapshot(
                &capture_current_authored_snapshot(&time_of_day, &environment, &manual),
            )
            .err();
        }
        if pause.is_some() {
            apply_time_of_day_dev_action(TimeOfDayDevAction::TogglePaused, &mut time_of_day);
        }
    }
}

pub fn handle_world_time_presets(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
    buttons: Query<(&Interaction, &DevWorldTimePresetButton), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (interaction, preset) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        match preset.preset {
            super::state::WorldTimePreset::Dawn => {
                apply_time_of_day_dev_action(TimeOfDayDevAction::SetDawn, &mut time_of_day);
            }
            super::state::WorldTimePreset::Noon => {
                apply_time_of_day_dev_action(TimeOfDayDevAction::SetNoon, &mut time_of_day);
            }
            super::state::WorldTimePreset::Midnight => {
                apply_time_of_day_dev_action(TimeOfDayDevAction::SetMidnight, &mut time_of_day);
            }
        }
    }
}

pub fn handle_world_skybox_selection(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut environment: ResMut<EnvironmentSettings>,
    mut ui_state: ResMut<WorldEnvironmentUiState>,
    buttons: Query<(&Interaction, &DevWorldSkyboxOption), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    let sets = list_registered_skybox_sets();
    for (interaction, option) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        if let Some(set_name) = sets.get(option.index) {
            environment.skybox_set = set_name.clone();
            ui_state.selected_skybox_index = option.index;
        }
    }
}

pub fn handle_world_slider_interaction(
    registry: Res<DevWindowRegistry>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    mut drag: ResMut<DevSliderDragState>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
    mut environment: ResMut<EnvironmentSettings>,
    mut manual: ResMut<EnvironmentManualLighting>,
    mut ui_state: ResMut<WorldEnvironmentUiState>,
    mut interactions: ParamSet<(
        Query<
            (&Interaction, &DevWidgetSliderValue),
            (Changed<Interaction>, Without<DevWidgetSliderTrack>),
        >,
        Query<
            (&DevWidgetSliderTrack, &Interaction, &RelativeCursorPosition),
            Without<DevWidgetSliderValue>,
        >,
    )>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }

    for (interaction, value_btn) in interactions.p0().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            if let Some(field) = EnvFieldId::from_u32(value_btn.id) {
                ui_state.focused_field = Some(field);
                dev_state.text_focus = DevTextFieldFocus::WorldEnvironmentNumeric;
                let spec = field.spec();
                let current = field.read(&time_of_day, &environment, &manual);
                ui_state
                    .draft_mut(field)
                    .sync_from_authoritative(current, spec.precision);
            }
        }
    }

    if mouse.just_pressed(MouseButton::Left) {
        for (track, interaction, relative) in interactions.p1().iter() {
            if *interaction != Interaction::Pressed {
                continue;
            }
            drag.field_id = Some(track.id);
            gate.block_gameplay_mouse = true;
            gate.block_camera_input = true;
            if let Some(field) = EnvFieldId::from_u32(track.id) {
                if let Some(norm) = slider_normalized_x(relative) {
                    let spec = field.spec();
                    let value = normalized_to_value(norm, spec.min, spec.max);
                    commit_field_value(
                        field,
                        value,
                        &mut time_of_day,
                        &mut environment,
                        &mut manual,
                        &mut ui_state,
                    );
                }
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        drag.field_id = None;
    }

    let Some(field_id) = drag.field_id else {
        return;
    };
    let Some(field) = EnvFieldId::from_u32(field_id) else {
        return;
    };

    for (track, _interaction, relative) in interactions.p1().iter() {
        if track.id != field_id {
            continue;
        }
        let Some(norm) = slider_normalized_x(relative) else {
            continue;
        };
        let spec = field.spec();
        let value = normalized_to_value(norm, spec.min, spec.max);
        commit_field_value(
            field,
            value,
            &mut time_of_day,
            &mut environment,
            &mut manual,
            &mut ui_state,
        );
        gate.block_camera_input = true;
        gate.block_gameplay_mouse = true;
    }
}

fn commit_field_value(
    field: EnvFieldId,
    value: f32,
    time_of_day: &mut TimeOfDaySettings,
    environment: &mut EnvironmentSettings,
    manual: &mut EnvironmentManualLighting,
    ui_state: &mut WorldEnvironmentUiState,
) {
    let spec = field.spec();
    let clamped = apply_numeric_bounds(value, Some(spec.min), Some(spec.max), true)
        .unwrap_or_else(|_| value.clamp(spec.min, spec.max));
    field.write(clamped, time_of_day, environment, manual);
    let snapshot = capture_current_authored_snapshot(time_of_day, environment, manual);
    ui_state.validation_error = validate_authored_snapshot(&snapshot).err();
}

pub fn handle_world_environment_numeric_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    registry: Res<DevWindowRegistry>,
    mut dev_state: ResMut<DevModeState>,
    mut ui_state: ResMut<WorldEnvironmentUiState>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
    mut environment: ResMut<EnvironmentSettings>,
    mut manual: ResMut<EnvironmentManualLighting>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    if dev_state.text_focus != DevTextFieldFocus::WorldEnvironmentNumeric {
        return;
    }
    let Some(field) = ui_state.focused_field else {
        return;
    };
    let spec = field.spec();

    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Escape) {
        if keyboard.just_pressed(KeyCode::Enter) {
            let draft = ui_state.draft_mut(field).text.clone();
            match parse_numeric_draft(&draft, spec.signed, true) {
                NumericParseResult::Valid(value) => {
                    commit_field_value(
                        field,
                        value,
                        &mut time_of_day,
                        &mut environment,
                        &mut manual,
                        &mut ui_state,
                    );
                }
                NumericParseResult::Invalid(_) => {
                    ui_state.set_error(format!("Invalid value for {}", spec.label));
                }
                NumericParseResult::Intermediate => {}
            }
        }
        ui_state.clear_focus();
        dev_state.text_focus = DevTextFieldFocus::None;
        return;
    }

    let draft = ui_state.draft_mut(field);
    if keyboard.just_pressed(KeyCode::Backspace) {
        draft.text.pop();
        return;
    }
    for key in keyboard.get_just_pressed() {
        let ch = key_to_numeric_char(*key, spec.signed);
        if let Some(ch) = ch {
            if draft.text.len() < 16 {
                draft.text.push(ch);
            }
        }
    }
}

fn key_to_numeric_char(key: KeyCode, signed: bool) -> Option<char> {
    match key {
        KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
        KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
        KeyCode::Minus | KeyCode::NumpadSubtract if signed => Some('-'),
        _ => None,
    }
}

pub fn focus_world_environment_numeric(
    mut dev_state: ResMut<DevModeState>,
    ui_state: Res<WorldEnvironmentUiState>,
) {
    if ui_state.focused_field.is_some() {
        dev_state.text_focus = DevTextFieldFocus::WorldEnvironmentNumeric;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::built_in_authored_snapshot;

    #[test]
    fn revert_preserves_runtime_clock() {
        let baseline = ProjectEnvironmentBaseline {
            snapshot: built_in_authored_snapshot(),
            load_status: ProjectDefaultsLoadStatus::LoadedFromFile,
            source_path: std::path::PathBuf::from(crate::environment::PROJECT_DEFAULTS_PATH),
        };
        let mut time = TimeOfDaySettings {
            time_hours: 4.5,
            day_length_seconds: 100.0,
            ..Default::default()
        };
        let mut env = EnvironmentSettings::default();
        let mut manual = EnvironmentManualLighting::default();
        let mut ui = WorldEnvironmentUiState::default();
        apply_revert(&baseline, &mut time, &mut env, &mut manual, &mut ui);
        assert!((time.time_hours - 4.5).abs() < f32::EPSILON);
        assert!(
            (time.day_length_seconds - baseline.snapshot.time_of_day.day_length_seconds).abs()
                < f32::EPSILON
        );
    }
}
