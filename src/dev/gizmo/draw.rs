//! Draw transform gizmo handles and collision preview (ADR-099).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::terrain::world_position_to_render_global;
use crate::world::{
    BuildingCatalog, DoodadCatalog, OCCUPANCY_CELL_SIZE_METERS, WorldConfig, WorldData,
    occupied_cells_for_footprint_yaw, resolve_doodad_collision,
};

use super::handles::{GizmoHandle, active_handles, policy_for_target};
use super::math::{GIZMO_HANDLE_LENGTH_FACTOR, apparent_gizmo_scale, oriented_axis};
use super::state::{DoodadPreviewPlacement, TransformEditState};
use super::tool::{GizmoCoordinateSpace, SelectedWorldObject};

/// Default vertical FOV for apparent-size heuristic when projection is unavailable.
const GIZMO_CAMERA_FOV_Y: f32 = std::f32::consts::FRAC_PI_4;

pub fn draw_transform_gizmo(
    dev_state: Res<crate::dev::DevModeState>,
    edit: Res<TransformEditState>,
    world: Res<WorldData>,
    catalog: Res<DoodadCatalog>,
    building_catalog: Res<BuildingCatalog>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<crate::terrain::TerrainRenderAssets>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::RtsCamera>>,
    mut gizmos: Gizmos,
) {
    if !dev_state.enabled || !edit.mode.is_transform() {
        return;
    }
    let Some(target) = edit.target else {
        return;
    };
    let policy = policy_for_target(target, &building_catalog, &world);
    let (anchor_render, rotation) = match target {
        SelectedWorldObject::Doodad(id) => {
            doodad_anchor(&world, &catalog, &config, id, &edit, &render_assets)
        }
        SelectedWorldObject::Building(id) => {
            building_anchor(&world, &config, id, &edit, &render_assets)
        }
        SelectedWorldObject::ItemPile(_) => return,
    };
    let Some(anchor) = anchor_render else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let fov = GIZMO_CAMERA_FOV_Y;
    let viewport_h = window.resolution.height();
    let gizmo_scale = apparent_gizmo_scale(camera_transform.translation(), anchor, fov, viewport_h);

    gizmos.sphere(anchor, 0.08 * gizmo_scale, Color::srgba(1.0, 1.0, 1.0, 0.9));

    let handles = active_handles(edit.mode, policy.capabilities, edit.coordinate_space);
    for handle in handles {
        let hovered = edit.hovered_handle == Some(handle);
        let active = edit.active_handle == Some(handle);
        let color = if active || hovered {
            handle.highlight_color()
        } else {
            handle.color()
        };
        draw_handle(
            &mut gizmos,
            handle,
            anchor,
            rotation,
            gizmo_scale,
            edit.coordinate_space,
            color,
        );
    }

    if let SelectedWorldObject::Doodad(id) = target {
        if let Some(preview) = edit.preview_placement {
            draw_collision_preview(
                &mut gizmos,
                &world,
                &catalog,
                id,
                preview,
                &config,
                &render_assets,
            );
        }
    }

    if !policy.can_commit {
        if let Some(reason) = policy.commit_blocked_reason {
            let _ = reason;
        }
    }
}

fn doodad_anchor(
    world: &WorldData,
    catalog: &DoodadCatalog,
    config: &WorldConfig,
    id: crate::world::DoodadId,
    edit: &TransformEditState,
    render_assets: &Option<Res<crate::terrain::TerrainRenderAssets>>,
) -> (Option<Vec3>, Quat) {
    let placement = edit.preview_placement.or_else(|| {
        world
            .get_doodad(id)
            .map(|r| DoodadPreviewPlacement::from_placement(r.placement))
    });
    let Some(placement) = placement else {
        return (None, Quat::IDENTITY);
    };
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let anchor =
        world_position_to_render_global(placement.position, config.chunk_layout(), vertical_scale);
    let rotation = placement.rotation_quat();
    let _ = catalog;
    (Some(anchor), rotation)
}

fn building_anchor(
    world: &WorldData,
    config: &WorldConfig,
    id: crate::world::BuildingId,
    edit: &TransformEditState,
    render_assets: &Option<Res<crate::terrain::TerrainRenderAssets>>,
) -> (Option<Vec3>, Quat) {
    let placement = edit.preview_placement.or_else(|| {
        world
            .get_building(id)
            .map(|r| super::state::building_preview_from_placement(r.placement))
    });
    let Some(placement) = placement else {
        return (None, Quat::IDENTITY);
    };
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let anchor =
        world_position_to_render_global(placement.position, config.chunk_layout(), vertical_scale);
    (Some(anchor), placement.rotation_quat())
}

fn draw_handle(
    gizmos: &mut Gizmos,
    handle: GizmoHandle,
    anchor: Vec3,
    object_rotation: Quat,
    scale: f32,
    space: GizmoCoordinateSpace,
    color: Color,
) {
    match handle {
        GizmoHandle::TranslateX | GizmoHandle::TranslateY | GizmoHandle::TranslateZ => {
            let axis = oriented_axis(handle.axis().unwrap(), object_rotation, space);
            let end = anchor + axis.normalize() * scale * GIZMO_HANDLE_LENGTH_FACTOR;
            gizmos.line(anchor, end, color);
            gizmos.sphere(end, scale * 0.07, color);
        }
        GizmoHandle::TranslateXY | GizmoHandle::TranslateXZ | GizmoHandle::TranslateYZ => {
            let normal = oriented_axis(handle.plane_normal().unwrap(), object_rotation, space);
            let half = scale * 0.18;
            let (u, v) = match handle {
                GizmoHandle::TranslateXY => (Vec3::X, Vec3::Y),
                GizmoHandle::TranslateXZ => (Vec3::X, Vec3::Z),
                GizmoHandle::TranslateYZ => (Vec3::Y, Vec3::Z),
                _ => unreachable!(),
            };
            let u = oriented_axis(u, object_rotation, space) * half;
            let v = oriented_axis(v, object_rotation, space) * half;
            let corners = [
                anchor - u - v,
                anchor + u - v,
                anchor + u + v,
                anchor - u + v,
            ];
            for i in 0..4 {
                gizmos.line(corners[i], corners[(i + 1) % 4], color);
            }
            let _ = normal;
        }
        GizmoHandle::RotateX | GizmoHandle::RotateY | GizmoHandle::RotateZ => {
            let normal = oriented_axis(handle.plane_normal().unwrap(), object_rotation, space);
            let radius = scale * 0.85;
            gizmos.circle(
                Isometry3d::new(anchor, Quat::from_rotation_arc(Vec3::Z, normal.normalize())),
                radius,
                color,
            );
        }
        GizmoHandle::ScaleX | GizmoHandle::ScaleY | GizmoHandle::ScaleZ => {
            let axis = oriented_axis(
                handle.axis().unwrap(),
                object_rotation,
                GizmoCoordinateSpace::Local,
            );
            let end = anchor + axis.normalize() * scale * 0.85;
            gizmos.line(anchor, end, color);
            gizmos.sphere(end, scale * 0.06, color);
        }
        GizmoHandle::ScaleUniform => {
            gizmos.sphere(anchor, scale * 0.1, color);
        }
    }
}

fn draw_collision_preview(
    gizmos: &mut Gizmos,
    world: &WorldData,
    catalog: &DoodadCatalog,
    id: crate::world::DoodadId,
    preview: DoodadPreviewPlacement,
    config: &WorldConfig,
    render_assets: &Option<Res<crate::terrain::TerrainRenderAssets>>,
) {
    let Some(record) = world.get_doodad(id) else {
        return;
    };
    let Some(definition) = catalog.get(&record.definition_id) else {
        return;
    };
    let mut trial = record.clone();
    trial.placement = preview.to_placement();
    let collision = resolve_doodad_collision(&trial, definition);
    if !collision.blocks_movement {
        return;
    }
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let global = preview.position.to_global(config.chunk_layout());
    let anchor_xz = Vec2::new(global.x, global.z);
    let cells =
        occupied_cells_for_footprint_yaw(&collision.shape, anchor_xz, collision.yaw_radians);
    let color = Color::srgba(1.0, 0.85, 0.2, 0.35);
    for cell in cells {
        let center_xz = cell.center_global();
        let center = Vec3::new(center_xz.x, global.y * vertical_scale + 0.05, center_xz.y);
        let half = OCCUPANCY_CELL_SIZE_METERS * 0.48;
        let corners = [
            center + Vec3::new(-half, 0.0, -half),
            center + Vec3::new(half, 0.0, -half),
            center + Vec3::new(half, 0.0, half),
            center + Vec3::new(-half, 0.0, half),
        ];
        for i in 0..4 {
            gizmos.line(corners[i], corners[(i + 1) % 4], color);
        }
    }
}
