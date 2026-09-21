use std::path::Path;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::dev::dev_mode::DevModeState;
use crate::dev::spawn_tools::dev_spawn_position_from_terrain_click;
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{SelectedUnits, cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    OriginCatalog, OriginDefinition, OriginId, WorldConfig, WorldData, capture_origin_member_from_unit,
    load_origins_from_ron, save_origins_to_ron, slugify_archetype_id, ORIGINS_RON_PATH,
};

use super::state::DevOriginEditorState;

#[derive(Component, Debug, Clone, Copy)]
pub enum OriginEditorButton {
    Prev,
    Next,
    AddFromSelection,
    OverwriteFromSelection,
    Delete,
    ConfirmDelete,
    CancelDelete,
    SetStartHere,
    FacingFromCamera,
    Save,
    Reload,
}

pub fn handle_origin_editor_buttons(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut editor: ResMut<DevOriginEditorState>,
    mut origins: ResMut<OriginCatalog>,
    selected_units: Res<SelectedUnits>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    camera: Query<&RtsCameraState, With<RtsCamera>>,
    buttons: Query<(&Interaction, &OriginEditorButton), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::OriginEditor) {
        return;
    }

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            OriginEditorButton::Prev => select_origin(&mut editor, &origins, -1),
            OriginEditorButton::Next => select_origin(&mut editor, &origins, 1),
            OriginEditorButton::AddFromSelection => {
                add_from_selection(&mut editor, &mut origins, &selected_units, &world, &config);
            }
            OriginEditorButton::OverwriteFromSelection => {
                overwrite_from_selection(
                    &mut editor,
                    &mut origins,
                    &selected_units,
                    &world,
                );
            }
            OriginEditorButton::Delete => {
                if origins.definitions().is_empty() {
                    editor.status_message = "No origin to delete".into();
                } else {
                    editor.pending_delete = true;
                    editor.status_message = "Confirm delete?".into();
                }
            }
            OriginEditorButton::ConfirmDelete => {
                confirm_delete(&mut editor, &mut origins);
            }
            OriginEditorButton::CancelDelete => {
                editor.pending_delete = false;
                editor.status_message = "Delete cancelled".into();
            }
            OriginEditorButton::SetStartHere => {
                editor.pending_start_pick = !editor.pending_start_pick;
                editor.status_message = if editor.pending_start_pick {
                    "Click terrain to set start".into()
                } else {
                    "Start pick cancelled".into()
                };
            }
            OriginEditorButton::FacingFromCamera => {
                set_facing_from_camera(&mut editor, &mut origins, &camera);
            }
            OriginEditorButton::Save => save_origins(&mut editor, &origins),
            OriginEditorButton::Reload => reload_origins(&mut editor, &mut origins),
        }
    }
}

pub fn handle_origin_editor_world_input(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut editor: ResMut<DevOriginEditorState>,
    mut origins: ResMut<OriginCatalog>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    world: Res<WorldData>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::OriginEditor) {
        return;
    }
    if !editor.pending_start_pick || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale) else {
        editor.status_message = "No terrain under cursor".into();
        return;
    };
    let Some(position) = dev_spawn_position_from_terrain_click(&world, click.world_position) else {
        editor.status_message = "Invalid terrain position".into();
        return;
    };
    let global = position.to_global(layout);
    mutate_selected_origin(&mut editor, &mut origins, |origin| {
        origin.start_x = global.x;
        origin.start_z = global.z;
    });
    editor.pending_start_pick = false;
    editor.status_message = format!("Start set to ({:.1}, {:.1})", global.x, global.z);
}

fn select_origin(editor: &mut DevOriginEditorState, origins: &OriginCatalog, delta: i32) {
    let count = origins.definitions().len();
    if count == 0 {
        editor.selected_index = 0;
        editor.status_message = "No origins loaded".into();
        return;
    }
    let next = (editor.selected_index as i32 + delta).rem_euclid(count as i32) as usize;
    editor.selected_index = next;
    editor.pending_delete = false;
    editor.pending_start_pick = false;
    let origin = &origins.definitions()[next];
    editor.sync_scratch_from_definition(&origin.display_name, &origin.description);
    editor.status_message = format!("Selected {}", origin.id.as_str());
}

fn add_from_selection(
    editor: &mut DevOriginEditorState,
    origins: &mut OriginCatalog,
    selected_units: &SelectedUnits,
    world: &WorldData,
    config: &WorldConfig,
) {
    let Some(unit_id) = selected_units.iter().next() else {
        editor.status_message = "Select a unit first".into();
        return;
    };
    let Some(member) = capture_origin_member_from_unit(world, unit_id, "Member".into(), Vec3::ZERO)
    else {
        editor.status_message = "Failed to capture unit".into();
        return;
    };
    let layout = config.chunk_layout();
    let global = world
        .get_unit(unit_id)
        .map(|record| record.placement.position.to_global(layout))
        .unwrap_or(Vec3::ZERO);
    let base_id = unique_origin_id(origins, &member.definition_id.as_str());
    let display_name = member.role_label.clone();
    let definition = OriginDefinition {
        id: OriginId::new(base_id.clone()),
        display_name: display_name.clone(),
        description: String::new(),
        members: vec![member],
        start_x: global.x,
        start_z: global.z,
        yaw_deg: 0.0,
    };
    if origins.upsert(definition).is_err() {
        editor.status_message = "Failed to add origin".into();
        return;
    }
    editor.selected_index = origins.definitions().len().saturating_sub(1);
    editor.dirty = true;
    editor.sync_scratch_from_definition(&display_name, "");
    editor.status_message = format!("Added origin `{}`", base_id);
}

fn overwrite_from_selection(
    editor: &mut DevOriginEditorState,
    origins: &mut OriginCatalog,
    selected_units: &SelectedUnits,
    world: &WorldData,
) {
    if origins.definitions().is_empty() {
        editor.status_message = "No origin selected".into();
        return;
    }
    let Some(unit_id) = selected_units.iter().next() else {
        editor.status_message = "Select a unit first".into();
        return;
    };
    let Some(member) = capture_origin_member_from_unit(world, unit_id, "Member".into(), Vec3::ZERO)
    else {
        editor.status_message = "Failed to capture unit".into();
        return;
    };
    let display_name = editor.scratch_name.clone();
    let description = editor.scratch_description.clone();
    mutate_selected_origin(editor, origins, |origin| {
        origin.display_name = display_name;
        origin.description = description;
        origin.members = vec![member];
    });
    editor.dirty = true;
    editor.status_message = "Origin overwritten from selection".into();
}

fn confirm_delete(editor: &mut DevOriginEditorState, origins: &mut OriginCatalog) {
    if !editor.pending_delete {
        return;
    }
    let definitions = origins.definitions();
    if definitions.is_empty() {
        editor.pending_delete = false;
        return;
    }
    let index = editor
        .selected_index
        .min(definitions.len().saturating_sub(1));
    let id = definitions[index].id.clone();
    if origins.remove(&id) {
        editor.dirty = true;
        editor.selected_index = editor
            .selected_index
            .min(origins.definitions().len().saturating_sub(1));
        if let Some(origin) = origins.get_index(editor.selected_index) {
            editor.sync_scratch_from_definition(&origin.display_name, &origin.description);
        }
        editor.status_message = format!("Deleted `{}`", id.as_str());
    }
    editor.pending_delete = false;
}

fn set_facing_from_camera(
    editor: &mut DevOriginEditorState,
    origins: &mut OriginCatalog,
    camera: &Query<&RtsCameraState, With<RtsCamera>>,
) {
    if origins.definitions().is_empty() {
        editor.status_message = "No origin selected".into();
        return;
    }
    let Some(state) = camera.iter().next() else {
        editor.status_message = "No RTS camera".into();
        return;
    };
    let yaw_deg = state.target_yaw.to_degrees();
    mutate_selected_origin(editor, origins, |origin| {
        origin.yaw_deg = yaw_deg;
    });
    editor.dirty = true;
    editor.status_message = format!("Facing set to {:.0}°", yaw_deg);
}

fn save_origins(editor: &mut DevOriginEditorState, origins: &OriginCatalog) {
    match save_origins_to_ron(origins, Path::new(ORIGINS_RON_PATH)) {
        Ok(path) => {
            editor.dirty = false;
            editor.status_message = format!("Saved to {}", path.display());
        }
        Err(error) => editor.status_message = format!("Save failed: {error}"),
    }
}

fn reload_origins(editor: &mut DevOriginEditorState, origins: &mut OriginCatalog) {
    match load_origins_from_ron(Path::new(ORIGINS_RON_PATH)) {
        Ok(catalog) => {
            *origins = catalog;
            editor.selected_index = 0;
            editor.dirty = false;
            editor.pending_delete = false;
            editor.pending_start_pick = false;
            if let Some(origin) = origins.get_index(0) {
                editor.sync_scratch_from_definition(&origin.display_name, &origin.description);
            }
            editor.status_message = "Reloaded origins".into();
        }
        Err(error) => editor.status_message = format!("Reload failed: {error}"),
    }
}

fn mutate_selected_origin(
    editor: &mut DevOriginEditorState,
    origins: &mut OriginCatalog,
    mutate: impl FnOnce(&mut OriginDefinition),
) {
    let definitions = origins.definitions();
    if definitions.is_empty() {
        return;
    }
    let index = editor
        .selected_index
        .min(definitions.len().saturating_sub(1));
    let mut definition = definitions[index].clone();
    mutate(&mut definition);
    if origins.upsert(definition).is_ok() {
        editor.dirty = true;
    }
}

fn unique_origin_id(origins: &OriginCatalog, seed: &str) -> String {
    let base = slugify_archetype_id(seed);
    let base = if base.is_empty() { "origin" } else { base.as_str() };
    if origins.get(&OriginId::new(base)).is_none() {
        return base.to_string();
    }
    for index in 2..1000 {
        let candidate = format!("{base}_{index}");
        if origins.get(&OriginId::new(&candidate)).is_none() {
            return candidate;
        }
    }
    format!("{base}_{}", origins.definitions().len() + 1)
}
