use bevy::prelude::*;

use crate::dev::dev_mode::{DevModeInputGate, DevModeState, DevTextFieldFocus};
use crate::dev::input::DevPanelUi;
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::world::{
    RoadNetwork, detach_junction, junction_id_for_endpoint, load_road_network_from_world_package,
    refresh_tee_branches_for_host, save_road_network, try_snap_endpoint, validate_road_network,
};

use super::domain::{
    cycle_road_style, delete_control_point, delete_road, finish_create_road, road_label,
};
use super::panel::DevRoadNameField;
use super::state::{RoadEditMode, RoadEditorUiState};

#[derive(Component, Debug, Clone, Copy)]
pub enum RoadEditorButton {
    Create,
    Finish,
    Cancel,
    InsertPoint,
    ExtendStart,
    ExtendEnd,
    DeletePoint,
    DeleteRoad,
    DetachJunction,
    CycleStyle,
    Save,
    Reload,
}

pub fn setup_road_editor_state(mut commands: Commands, network: Res<RoadNetwork>) {
    let mut editor = RoadEditorUiState::default();
    editor.sync_baseline_from(&network);
    commands.insert_resource(editor);
}

pub fn handle_road_editor_buttons(
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    mut editor: ResMut<RoadEditorUiState>,
    mut network: ResMut<RoadNetwork>,
    buttons: Query<(&Interaction, &RoadEditorButton), (Changed<Interaction>, With<DevPanelUi>)>,
    name_fields: Query<&Interaction, (Changed<Interaction>, With<DevRoadNameField>)>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Roads) {
        return;
    }

    for interaction in &name_fields {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.text_focus = DevTextFieldFocus::RoadName;
        }
    }

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        match button {
            RoadEditorButton::Create => {
                dev_state.settlement_placement_armed = false;
                dev_state.cancel_placement_tool();
                editor.begin_create(&mut network);
            }
            RoadEditorButton::Finish => finish_active_operation(&mut editor, &mut network),
            RoadEditorButton::Cancel => editor.cancel_active(&mut network),
            RoadEditorButton::InsertPoint => {
                if let Err(message) = editor.begin_insert_point(&mut network) {
                    editor.status_message = message;
                }
            }
            RoadEditorButton::ExtendStart => {
                if let Err(message) = editor.begin_extend(&mut network, true) {
                    editor.status_message = message;
                }
            }
            RoadEditorButton::ExtendEnd => {
                if let Err(message) = editor.begin_extend(&mut network, false) {
                    editor.status_message = message;
                }
            }
            RoadEditorButton::DeletePoint => delete_selected_point(&mut editor, &mut network),
            RoadEditorButton::DeleteRoad => {
                if editor.pending_delete_confirmation {
                    if let Some(road_id) = editor.selected_road_id.clone() {
                        delete_road(&mut network, &road_id);
                        editor.selected_road_id = None;
                        editor.selected_point_index = None;
                        editor.pending_delete_confirmation = false;
                        editor.mark_dirty(format!("Deleted road {}", road_id));
                    }
                } else if editor.selected_road_id.is_some() {
                    editor.pending_delete_confirmation = true;
                    editor.status_message = "Click Delete Road again to confirm".into();
                } else {
                    editor.status_message = "Select a road to delete".into();
                }
            }
            RoadEditorButton::DetachJunction => detach_selected_junction(&mut editor, &mut network),
            RoadEditorButton::CycleStyle => cycle_selected_style(&mut editor, &mut network),
            RoadEditorButton::Save => save_network(&mut editor, &network),
            RoadEditorButton::Reload => reload_network(&mut editor, &mut network),
        }
    }
}

pub fn handle_road_editor_keyboard(
    keyboard: &ButtonInput<KeyCode>,
    dev_state: &mut DevModeState,
    editor: &mut RoadEditorUiState,
    network: &mut RoadNetwork,
) {
    if dev_state.text_focus == DevTextFieldFocus::RoadName {
        if keyboard.just_pressed(KeyCode::Enter) {
            apply_name_input(editor, network);
            dev_state.clear_text_focus();
            return;
        }
        if keyboard.just_pressed(KeyCode::Backspace) {
            editor.name_input.pop();
        }
        for key in keyboard.get_just_pressed() {
            if let Some(ch) = road_name_char(*key) {
                editor.name_input.push(ch);
            }
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        finish_active_operation(editor, network);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        editor.cancel_active(network);
    }
}

fn finish_active_operation(editor: &mut RoadEditorUiState, network: &mut RoadNetwork) {
    match editor.mode {
        RoadEditMode::Create => {
            let dirty_before = editor.take_create_transaction();
            let display_name = editor.create_display_name.clone();
            let style = editor.create_style;
            let points = editor.create_points.clone();
            match finish_create_road(network, display_name, style, points) {
                Ok(road_id) => {
                    editor.selected_road_id = Some(road_id.clone());
                    editor.selected_point_index = None;
                    editor.clear_tool_state();
                    editor.mark_dirty(format!("Created road {}", road_id));
                }
                Err(message) => {
                    if let Some(dirty_before) = dirty_before {
                        editor.dirty = dirty_before;
                        editor.transaction = Some(super::transaction::RoadToolTransaction {
                            dirty_before,
                            kind: super::transaction::RoadToolTransactionKind::Create,
                        });
                        editor.mode = RoadEditMode::Create;
                    }
                    editor.status_message = message;
                }
            }
        }
        RoadEditMode::ExtendStart | RoadEditMode::ExtendEnd => {
            let from_start = editor.mode == RoadEditMode::ExtendStart;
            if let Some((road_id, original_road, dirty_before)) = editor.take_extend_transaction() {
                let changed = network
                    .roads
                    .get(&road_id)
                    .is_some_and(|road| road.control_points != original_road.control_points);
                if changed {
                    if let Some(road) = network.roads.get(&road_id) {
                        let endpoint = if from_start {
                            road.control_points.first().map(|point| point.xz())
                        } else {
                            road.control_points.last().map(|point| point.xz())
                        };
                        if let Some(xz) = endpoint {
                            if let Err(message) =
                                try_snap_endpoint(network, &road_id, from_start, xz)
                            {
                                editor.status_message = message;
                                editor.transaction = Some(super::transaction::RoadToolTransaction {
                                    dirty_before,
                                    kind: super::transaction::RoadToolTransactionKind::Extend {
                                        road_id,
                                        original_road,
                                    },
                                });
                                editor.mode = if from_start {
                                    RoadEditMode::ExtendStart
                                } else {
                                    RoadEditMode::ExtendEnd
                                };
                                return;
                            }
                            refresh_tee_branches_for_host(network, &road_id);
                        }
                    }
                    editor.clear_tool_state();
                    editor.mark_dirty(format!("Finished road extension for {}", road_id));
                } else {
                    editor.clear_tool_state();
                    editor.dirty = dirty_before;
                    editor.status_message = "No extension points added".into();
                }
            } else {
                editor.status_message = "Nothing to finish".into();
            }
        }
        RoadEditMode::InsertPoint => {
            editor.clear_tool_state();
            editor.status_message = "Nothing to finish — click a road segment to insert".into();
        }
        RoadEditMode::Inactive => editor.status_message = "Nothing to finish".into(),
    }
}

fn delete_selected_point(editor: &mut RoadEditorUiState, network: &mut RoadNetwork) {
    let Some(road_id) = editor.selected_road_id.clone() else {
        editor.status_message = "Select a road point to delete".into();
        return;
    };
    let Some(index) = editor.selected_point_index else {
        editor.status_message = "Select a control point to delete".into();
        return;
    };
    let road = network.roads.get_mut(&road_id);
    let Some(road) = road else {
        editor.status_message = "Selected road no longer exists".into();
        return;
    };
    match delete_control_point(road, index) {
        Ok(()) => {
            editor.selected_point_index = None;
            editor.mark_dirty(format!("Deleted point from {}", road_id));
        }
        Err(message) => editor.status_message = message,
    }
}

fn detach_selected_junction(editor: &mut RoadEditorUiState, network: &mut RoadNetwork) {
    let Some(road_id) = editor.selected_road_id.clone() else {
        editor.status_message = "Select a road endpoint attached to a junction".into();
        return;
    };
    let Some(index) = editor.selected_point_index else {
        editor.status_message = "Select an attached endpoint to detach its junction".into();
        return;
    };
    let road = network.roads.get(&road_id);
    let Some(road) = road else {
        editor.status_message = "Selected road no longer exists".into();
        return;
    };
    let is_start = index == 0;
    let is_end = index + 1 == road.control_points.len();
    if !is_start && !is_end {
        editor.status_message = "Only attached endpoints can detach a junction".into();
        return;
    }
    let junction_id = junction_id_for_endpoint(road, is_start);
    let Some(junction_id) = junction_id else {
        editor.status_message = "Selected endpoint is not attached to a junction".into();
        return;
    };
    match detach_junction(network, &junction_id) {
        Ok(()) => editor.mark_dirty(format!("Detached junction {}", junction_id)),
        Err(message) => editor.status_message = message,
    }
}

fn cycle_selected_style(editor: &mut RoadEditorUiState, network: &mut RoadNetwork) {
    if editor.mode == RoadEditMode::Create {
        editor.create_style = cycle_road_style(editor.create_style);
        editor.status_message = "Updated draft road style".into();
        return;
    }
    let Some(road_id) = editor.selected_road_id.clone() else {
        editor.status_message = "Select a road to change style".into();
        return;
    };
    if let Some(road) = network.roads.get_mut(&road_id) {
        road.style = cycle_road_style(road.style);
        editor.mark_dirty(format!("Updated style for {}", road_label(road)));
    }
}

fn apply_name_input(editor: &mut RoadEditorUiState, network: &mut RoadNetwork) {
    let Some(road_id) = editor.selected_road_id.clone() else {
        if editor.mode == RoadEditMode::Create {
            editor.create_display_name = editor.name_input.clone();
            editor.status_message = "Updated draft road name".into();
        }
        return;
    };
    if let Some(road) = network.roads.get_mut(&road_id) {
        road.display_name = editor.name_input.clone();
        editor.mark_dirty(format!("Renamed road {}", road.id));
    }
}

fn save_network(editor: &mut RoadEditorUiState, network: &RoadNetwork) {
    if let Err(error) = validate_road_network(network) {
        editor.status_message = format!("Save failed validation: {error}");
        return;
    }
    match save_road_network(network) {
        Ok(()) => {
            editor.sync_baseline_from(network);
            editor.status_message = "Saved road network".into();
        }
        Err(error) => editor.status_message = format!("Save failed: {error}"),
    }
}

fn road_name_char(key: KeyCode) -> Option<char> {
    match key {
        KeyCode::Space => Some(' '),
        KeyCode::Minus => Some('-'),
        KeyCode::Period => Some('.'),
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
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
        _ => None,
    }
}

fn reload_network(editor: &mut RoadEditorUiState, network: &mut RoadNetwork) {
    editor.cancel_active(network);
    match load_road_network_from_world_package(crate::world::DEFAULT_WORLD_PACKAGE_DIR) {
        Ok(loaded) => {
            editor.sync_baseline_from(&loaded);
            *network = loaded;
            editor.selected_road_id = None;
            editor.selected_point_index = None;
            editor.clear_tool_state();
            editor.status_message = "Reloaded road network from disk".into();
        }
        Err(error) => editor.status_message = format!("Reload failed: {error}"),
    }
}
