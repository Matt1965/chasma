use bevy::prelude::*;

use crate::camera::RtsCamera;
use crate::dev::dev_mode::DevModeState;
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{
    Road, RoadNetwork, WorldConfig, WorldData, WorldPosition,
    derive_ground_crossings, junction_world_position, sample_road_polyline,
};

use super::domain::{SPLINE_SAMPLE_SPACING_M, style_debug_color};
use super::state::{RoadEditMode, RoadEditorUiState};

const VISUAL_Y_OFFSET: f32 = 0.12;

pub fn draw_road_editor_overlay(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    editor: Res<RoadEditorUiState>,
    network: Res<RoadNetwork>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut gizmos: Gizmos,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Roads) {
        return;
    }

    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let layout = config.chunk_layout();

    for road in network.roads.values() {
        let selected = editor
            .selected_road_id
            .as_ref()
            .is_some_and(|id| id == &road.id);
        draw_road_gizmos(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            road,
            selected,
            editor.selected_point_index,
            &editor.selected_road_id,
        );
    }

    for junction_id in network.junctions.keys() {
        if let Some(position) = junction_world_position(&network, junction_id) {
            let world_pos = terrain_vec3(&world, layout, vertical_scale, position);
            gizmos.sphere(
                Isometry3d::from_translation(world_pos),
                0.65,
                Color::srgba(1.0, 0.55, 0.15, 0.95),
            );
        }
    }

    for crossing in derive_ground_crossings(&network) {
        if !crossing.connected {
            continue;
        }
        let world_pos = terrain_vec3(&world, layout, vertical_scale, crossing.position);
        gizmos.sphere(
            Isometry3d::from_translation(world_pos),
            0.5,
            Color::srgba(0.95, 0.35, 0.95, 0.9),
        );
    }

    if let Some(candidate) = &editor.snap_preview {
        let world_pos = terrain_vec3(&world, layout, vertical_scale, candidate.position);
        gizmos.sphere(
            Isometry3d::from_translation(world_pos),
            0.55,
            Color::srgba(0.2, 1.0, 0.45, 0.95),
        );
    }

    if editor.mode == RoadEditMode::Create && !editor.create_points.is_empty() {
        let draft = Road {
            id: crate::world::RoadId::new("__draft__"),
            display_name: editor.create_display_name.clone(),
            style: editor.create_style,
            style_overrides: Default::default(),
            control_points: editor.create_points.clone(),
            start_attachment: None,
            end_attachment: None,
            tee_attachments: Vec::new(),
        };
        draw_road_gizmos(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            &draft,
            true,
            None,
            &None,
        );
    }

    let _ = camera;
}

fn draw_road_gizmos(
    gizmos: &mut Gizmos,
    world: &WorldData,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    road: &Road,
    selected: bool,
    selected_point: Option<usize>,
    selected_road_id: &Option<crate::world::RoadId>,
) {
    let color = style_debug_color(road.style);
    let width = road.style_overrides.width_m.unwrap_or(match road.style {
        crate::world::RoadStyleId::Trail => 2.5,
        crate::world::RoadStyleId::DirtRoad => 4.0,
        crate::world::RoadStyleId::MajorRoad => 6.0,
    });

    if road.control_points.len() >= 2 {
        let samples = sample_road_polyline(road, SPLINE_SAMPLE_SPACING_M);
        let points = samples
            .iter()
            .map(|sample| terrain_vec3(world, layout, vertical_scale, sample.position))
            .collect::<Vec<_>>();
        for window in points.windows(2) {
            gizmos.line(window[0], window[1], color);
        }
        draw_width_corridor(gizmos, &samples, world, layout, vertical_scale, width, color);
    }

    for (index, point) in road.control_points.iter().enumerate() {
        let point_selected = selected
            && selected_road_id.as_ref().is_some_and(|id| *id == road.id)
            && selected_point == Some(index);
        let position = terrain_vec3(world, layout, vertical_scale, point.xz());
        let point_color = if point_selected {
            Color::srgba(1.0, 0.92, 0.2, 1.0)
        } else if selected {
            Color::srgba(0.95, 0.95, 1.0, 0.95)
        } else {
            Color::srgba(0.7, 0.8, 0.95, 0.9)
        };
        gizmos.sphere(
            Isometry3d::from_translation(position),
            0.35,
            point_color,
        );
    }
}

fn draw_width_corridor(
    gizmos: &mut Gizmos,
    samples: &[crate::world::RoadSplineSample],
    world: &WorldData,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    width_m: f32,
    _color: Color,
) {
    let half = width_m * 0.5;
    let edge_color = Color::srgba(0.75, 0.75, 0.8, 0.35);
    for sample in samples {
        let tangent = sample.tangent.normalize_or_zero();
        if tangent.length_squared() <= f32::EPSILON {
            continue;
        }
        let normal = Vec2::new(-tangent.y, tangent.x);
        let left = sample.position + normal * half;
        let right = sample.position - normal * half;
        let left_pos = terrain_vec3(world, layout, vertical_scale, left);
        let right_pos = terrain_vec3(world, layout, vertical_scale, right);
        gizmos.line(left_pos, right_pos, edge_color);
    }
}

fn terrain_vec3(
    world: &WorldData,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    xz: Vec2,
) -> Vec3 {
    let candidate = WorldPosition::from_global(Vec3::new(xz.x, 0.0, xz.y), layout);
    let grounded = crate::world::ground_world_position(world, candidate).unwrap_or(candidate);
    let render = world_position_to_render_global(grounded, layout, vertical_scale);
    render + Vec3::Y * VISUAL_Y_OFFSET
}
