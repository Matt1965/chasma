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
use crate::world::{RoadControlPoint, RoadNetwork, WorldConfig, WorldData};

use super::domain::{
    insert_control_point, move_control_point, pick_control_point_at, pick_road_at,
    pick_segment_for_insert, extend_road_end, extend_road_start,
};
use super::state::{RoadEditMode, RoadEditorUiState, road_editor_owns_world_pointer};

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

    let left_pressed = mouse_buttons.just_pressed(MouseButton::Left);
    let left_released = mouse_buttons.just_released(MouseButton::Left);
    let left_held = mouse_buttons.pressed(MouseButton::Left);

    if editor.dragging_point.is_some() {
        gate.block_gameplay_mouse = true;
        if left_released {
            editor.dragging_point = None;
            editor.mark_dirty("Moved control point");
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
                    if let Some(road) = network.roads.get_mut(&road_id) {
                        let _ = move_control_point(road, index, xz);
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
                if let Some(road) = network.roads.get_mut(&road_id) {
                    extend_road_start(road, RoadControlPoint::new(xz.x, xz.y));
                    editor.mark_dirty(format!("Extended start of {}", road_id));
                }
            }
        }
        RoadEditMode::ExtendEnd => {
            if let Some(road_id) = editor.selected_road_id.clone() {
                if let Some(road) = network.roads.get_mut(&road_id) {
                    extend_road_end(road, RoadControlPoint::new(xz.x, xz.y));
                    editor.mark_dirty(format!("Extended end of {}", road_id));
                }
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
                    editor.mode = RoadEditMode::Inactive;
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
