//! Gizmo handle picking (ADR-099).
//!
//! Screen-space picking is used so handles match what you see on screen and stay
//! clickable when mesh geometry sits in front of the gizmo along the view ray.

use bevy::camera::Camera;
use bevy::prelude::*;

use super::handles::{GizmoHandle, active_handles};
use super::math::{GIZMO_HANDLE_LENGTH_FACTOR, oriented_axis};
use super::state::TransformEditState;
use super::tool::{DevTool, GizmoCoordinateSpace};
use crate::units::input::world_position_to_screen;
use crate::world::authoring_transform::TransformCapabilities;

/// Scale handles are always drawn in local space (see `draw.rs`); picking must match.
const SCALE_HANDLE_SPACE: GizmoCoordinateSpace = GizmoCoordinateSpace::Local;

/// Screen-space pick tolerances (pixels). Slightly generous so handles are easy to grab.
fn pick_threshold_px(handle: GizmoHandle) -> f32 {
    match handle {
        GizmoHandle::TranslateX
        | GizmoHandle::TranslateY
        | GizmoHandle::TranslateZ => 22.0,
        GizmoHandle::TranslateXY | GizmoHandle::TranslateXZ | GizmoHandle::TranslateYZ => 26.0,
        GizmoHandle::RotateX | GizmoHandle::RotateY | GizmoHandle::RotateZ => 20.0,
        GizmoHandle::ScaleX | GizmoHandle::ScaleY | GizmoHandle::ScaleZ => 24.0,
        GizmoHandle::ScaleUniform => 30.0,
    }
}

/// Pick the nearest gizmo handle under the cursor.
pub fn pick_gizmo_handle(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
    anchor: Vec3,
    object_rotation: Quat,
    gizmo_scale: f32,
    tool: DevTool,
    caps: TransformCapabilities,
    space: GizmoCoordinateSpace,
) -> Option<GizmoHandle> {
    let anchor_screen = world_position_to_screen(anchor, camera, camera_transform)?;
    let handles = active_handles(tool, caps, space);
    let mut best: Option<(f32, GizmoHandle)> = None;

    for handle in handles {
        let Some(dist) = handle_screen_pick_distance(
            camera,
            camera_transform,
            cursor,
            anchor,
            anchor_screen,
            object_rotation,
            gizmo_scale,
            handle,
            space,
        ) else {
            continue;
        };
        let threshold = pick_threshold_px(handle);
        if dist <= threshold && best.map(|(best_d, _)| dist < best_d).unwrap_or(true) {
            best = Some((dist, handle));
        }
    }
    best.map(|(_, handle)| handle)
}

fn handle_screen_pick_distance(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
    anchor: Vec3,
    anchor_screen: Vec2,
    object_rotation: Quat,
    gizmo_scale: f32,
    handle: GizmoHandle,
    space: GizmoCoordinateSpace,
) -> Option<f32> {
    match handle {
        GizmoHandle::TranslateX | GizmoHandle::TranslateY | GizmoHandle::TranslateZ => {
            let axis = oriented_axis(handle.axis()?, object_rotation, space);
            let end = anchor + axis.normalize() * gizmo_scale * GIZMO_HANDLE_LENGTH_FACTOR;
            axis_handle_screen_distance(camera, camera_transform, cursor, anchor_screen, end)
        }
        GizmoHandle::TranslateXY | GizmoHandle::TranslateXZ | GizmoHandle::TranslateYZ => {
            let half = gizmo_scale * 0.18;
            let (u_axis, v_axis) = plane_axes(handle)?;
            let u = oriented_axis(u_axis, object_rotation, space) * half;
            let v = oriented_axis(v_axis, object_rotation, space) * half;
            let corners = [
                anchor - u - v,
                anchor + u - v,
                anchor + u + v,
                anchor - u + v,
            ];
            let mut min_dist = f32::MAX;
            for i in 0..4 {
                let a = world_position_to_screen(corners[i], camera, camera_transform)?;
                let b = world_position_to_screen(corners[(i + 1) % 4], camera, camera_transform)?;
                min_dist = min_dist.min(distance_point_to_segment_2d(cursor, a, b));
            }
            Some(min_dist)
        }
        GizmoHandle::RotateX | GizmoHandle::RotateY | GizmoHandle::RotateZ => {
            let normal = oriented_axis(handle.plane_normal()?, object_rotation, space);
            let ref_dir = if normal.y.abs() > 0.9 {
                Vec3::X
            } else {
                Vec3::Y
            };
            let tangent = normal.cross(ref_dir).normalize_or_zero();
            if tangent.length_squared() < 1e-8 {
                return None;
            }
            let ring_point = anchor + tangent * gizmo_scale * 0.85;
            let ring_screen = world_position_to_screen(ring_point, camera, camera_transform)?;
            let screen_radius = anchor_screen.distance(ring_screen);
            Some((cursor.distance(anchor_screen) - screen_radius).abs())
        }
        GizmoHandle::ScaleX | GizmoHandle::ScaleY | GizmoHandle::ScaleZ => {
            let axis = oriented_axis(handle.axis()?, object_rotation, SCALE_HANDLE_SPACE);
            let end = anchor + axis.normalize() * gizmo_scale * 0.85;
            axis_handle_screen_distance(camera, camera_transform, cursor, anchor_screen, end)
        }
        GizmoHandle::ScaleUniform => Some(cursor.distance(anchor_screen)),
    }
}

fn axis_handle_screen_distance(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
    anchor_screen: Vec2,
    end: Vec3,
) -> Option<f32> {
    let end_screen = world_position_to_screen(end, camera, camera_transform)?;
    let line_dist = distance_point_to_segment_2d(cursor, anchor_screen, end_screen);
    let end_dist = cursor.distance(end_screen);
    Some(line_dist.min(end_dist))
}

fn distance_point_to_segment_2d(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-8 {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

fn plane_axes(handle: GizmoHandle) -> Option<(Vec3, Vec3)> {
    match handle {
        GizmoHandle::TranslateXY => Some((Vec3::X, Vec3::Y)),
        GizmoHandle::TranslateXZ => Some((Vec3::X, Vec3::Z)),
        GizmoHandle::TranslateYZ => Some((Vec3::Y, Vec3::Z)),
        _ => None,
    }
}

pub fn gizmo_has_priority(edit: &TransformEditState, has_target: bool) -> bool {
    has_target && edit.mode.is_transform()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_to_segment_midpoint() {
        let dist = distance_point_to_segment_2d(Vec2::new(0.5, 1.0), Vec2::ZERO, Vec2::new(1.0, 0.0));
        assert!((dist - 1.0).abs() < 1e-4);
    }
}
