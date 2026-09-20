use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::inspector::BlueprintInspectionState;
use crate::dev::input::DevPanelHoverState;
use crate::dev::spawn_tools::dev_spawn_position_from_terrain_click;
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    RoadControlPoint, RoadNetwork, WorldConfig, WorldData, find_snap_candidate,
    finalize_endpoint_drag, is_road_endpoint_index, move_connected_endpoint,

};

use super::domain::{
    insert_control_point, pick_control_point_at, pick_road_at, pick_segment_for_insert,
    extend_road_end, extend_road_start,
};
use super::state::{RoadEditMode, RoadEditorUiState, road_editor_owns_world_pointer};

pub fn update_road_editor_snap_preview(
    registry: Res<DevWindowRegistry>,
    dev_state: Res<DevModeState>,
    panel_hovered: Res<DevPanelHoverState>,
    mut editor: ResMut<RoadEditorUiState>,
    network: Res<RoadNetwork>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    world: Res<WorldData>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Roads) {
        editor.snap_preview = None;
        return;
    }
    if panel_hovered.hovered {
        editor.snap_preview = None;
        return;
    }

    let Some(xz) = terrain_xz_pick(&windows, &camera, &config, render_assets.as_deref(), &world)
    else {
        editor.snap_preview = None;
        return;
    };

    editor.snap_preview = match editor.mode {
        RoadEditMode::Create => {
            if editor.create_points.is_empty() {
                None
            } else {
                find_snap_candidate(network.as_ref(), &crate::world::RoadId::new("__draft__"), false, xz)
            }
        }
        RoadEditMode::ExtendStart => editor
            .selected_road_id
            .as_ref()
            .and_then(|road_id| find_snap_candidate(network.as_ref(), road_id, true, xz)),
        RoadEditMode::ExtendEnd => editor
            .selected_road_id
            .as_ref()
            .and_then(|road_id| find_snap_candidate(network.as_ref(), road_id, false, xz)),
        RoadEditMode::Inactive => {
            if let Some((road_id, index)) = editor.dragging_point.clone() {
                if network
                    .roads
                    .get(&road_id)
                    .is_some_and(|road| is_road_endpoint_index(index, road.control_points.len()))
                {
                    let is_start = index == 0;
                    find_snap_candidate(network.as_ref(), &road_id, is_start, xz)
                } else {
                    None
                }
            } else {
                None
            }
        }
        RoadEditMode::InsertPoint => None,
    };
}

pub fn handle_road_editor_world_input(
    registry: Res<DevWindowRegistry>,
    dev_state: Res<DevModeState>,
    mut gate: ResMut<DevModeInputGate>,
    panel_hovered: Res<DevPanelHoverState>,
    blueprint_inspection: Res<BlueprintInspectionState>,
    mut editor: ResMut<RoadEditorUiState>,
    mut network: ResMut<RoadNetwork>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    world: Res<WorldData>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    if blueprint_inspection.editing {
        return;
    }
    if !registry.window_active(dev_state.enabled, DevWindowId::Roads) {
        return;
    }
    if !road_editor_owns_world_pointer(
        dev_state.enabled,
        registry.is_visible(DevWindowId::Roads),
        panel_hovered.hovered,
        &*editor,
    ) {
        return;
    }

    let right_pressed = mouse_buttons.just_pressed(MouseButton::Right);
    let left_pressed = mouse_buttons.just_pressed(MouseButton::Left);
    let left_released = mouse_buttons.just_released(MouseButton::Left);
    let left_held = mouse_buttons.pressed(MouseButton::Left);

    if right_pressed {
        gate.block_gameplay_mouse = true;
        if editor.has_modal_tool_active() {
            editor.cancel_active(&mut network);
            return;
        }
        if editor.selected_road_id.is_some() || editor.selected_point_index.is_some() {
            editor.selected_road_id = None;
            editor.selected_point_index = None;
            editor.pending_delete_confirmation = false;
            editor.status_message = "Selection cleared".into();
        }
        return;
    }

    if editor.dragging_point.is_some() {
        gate.block_gameplay_mouse = true;
        if left_released {
            if let Some((road_id, index)) = editor.dragging_point.clone() {
                if let Some(xz) = terrain_xz_pick(
                    &windows,
                    &camera,
                    &config,
                    render_assets.as_deref(),
                    &world,
                ) {
                    if let Err(message) = finalize_endpoint_drag(&mut network, &road_id, index, xz) {
                        editor.status_message = message;
                    } else {
                        editor.mark_dirty("Moved control point");
                    }
                }
            }
            editor.dragging_point = None;
            editor.snap_preview = None;
            return;
        }
        if left_held {
            if let Some((road_id, index)) = editor.dragging_point.clone() {
                if let Some(xz) = terrain_xz_pick(
                    &windows,
                    &camera,
                    &config,
                    render_assets.as_deref(),
                    &world,
                ) {
                    if let Err(message) =
                        move_connected_endpoint(&mut network, &road_id, index, xz)
                    {
                        editor.status_message = message;
                    }
                }
            }
            gate.spawn_handled_this_frame = true;
        }
        return;
    }

    if !left_pressed {
        return;
    }

    let Some(xz) = terrain_xz_pick(&windows, &camera, &config, render_assets.as_deref(), &world)
    else {
        return;
    };

    gate.block_gameplay_mouse = true;
    gate.spawn_handled_this_frame = true;

    match editor.mode {
        RoadEditMode::Create => {
            editor
                .create_points
                .push(RoadControlPoint::new(xz.x, xz.y));
            editor.status_message = format!(
                "Create road — {} point(s); click Finish when done",
                editor.create_points.len()
            );
        }
        RoadEditMode::ExtendStart => {
            if let Some(road_id) = editor.selected_road_id.clone() {
                extend_road_start(
                    network
                        .roads
                        .get_mut(&road_id)
                        .expect("selected road"),
                    RoadControlPoint::new(xz.x, xz.y),
                );
                editor.status_message = format!(
                    "Extend start — added point on {}; click Finish to commit",
                    road_id
                );
            }
        }
        RoadEditMode::ExtendEnd => {
            if let Some(road_id) = editor.selected_road_id.clone() {
                extend_road_end(
                    network
                        .roads
                        .get_mut(&road_id)
                        .expect("selected road"),
                    RoadControlPoint::new(xz.x, xz.y),
                );
                editor.status_message = format!(
                    "Extend end — added point on {}; click Finish to commit",
                    road_id
                );
            }
        }
        RoadEditMode::InsertPoint => {
            if let Some(hit) = pick_segment_for_insert(&network, xz) {
                if let Ok(index) = insert_control_point(
                    &mut network,
                    &hit.road_id,
                    hit.segment_index,
                    hit.position,
                ) {
                    editor.selected_road_id = Some(hit.road_id);
                    editor.selected_point_index = Some(index);
                    editor.clear_tool_state();
                    editor.mark_dirty("Inserted control point");
                }
            } else {
                editor.status_message = "No road segment near click".into();
            }
        }
        RoadEditMode::Inactive => handle_select_or_drag(&mut editor, &network, xz),
    }
}

fn handle_select_or_drag(
    editor: &mut RoadEditorUiState,
    network: &RoadNetwork,
    xz: Vec2,
) {
    if let Some(road_id) = editor.selected_road_id.clone() {
        if let Some(road) = network.roads.get(&road_id) {
            if let Some(index) = pick_control_point_at(road, xz) {
                editor.selected_point_index = Some(index);
                editor.dragging_point = Some((road_id.clone(), index));
                editor.name_input = road.display_name.clone();
                editor.status_message = format!("Dragging point {} on {}", index + 1, road_id);
                return;
            }
        }
    }

    if let Some(road_id) = pick_road_at(network, xz) {
        editor.selected_road_id = Some(road_id.clone());
        editor.selected_point_index = None;
        editor.pending_delete_confirmation = false;
        if let Some(road) = network.roads.get(&road_id) {
            editor.name_input = road.display_name.clone();
            editor.status_message = format!("Selected road {}", road_id);
            if let Some(index) = pick_control_point_at(road, xz) {
                editor.selected_point_index = Some(index);
                editor.dragging_point = Some((road_id, index));
            }
        }
        return;
    }

    editor.status_message = "No road near click".into();
}

fn terrain_xz_pick(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    config: &WorldConfig,
    render_assets: Option<&TerrainRenderAssets>,
    world: &WorldData,
) -> Option<Vec2> {
    let Some(ray) = cursor_world_ray(windows, camera) else {
        return None;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets.map(|assets| assets.vertical_scale).unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, world, layout, vertical_scale) else {
        return None;
    };
    let Some(position) = dev_spawn_position_from_terrain_click(world, click.world_position) else {
        return None;
    };
    let global = position.to_global(layout);
    Some(Vec2::new(global.x, global.z))
}
