//! Gizmo handle picking (ADR-099).

use bevy::prelude::*;

use super::handles::{GizmoHandle, active_handles};
use super::math::{
    GIZMO_HANDLE_LENGTH_FACTOR, oriented_axis, pick_threshold, point_ray_distance,
    ray_plane_intersection, ray_segment_distance,
};
use super::state::TransformEditState;
use super::tool::{DevTool, GizmoCoordinateSpace};
use crate::world::authoring_transform::TransformCapabilities;

/// Pick the nearest gizmo handle along `ray`.
pub fn pick_gizmo_handle(
    ray: &Ray3d,
    anchor: Vec3,
    object_rotation: Quat,
    gizmo_scale: f32,
    tool: DevTool,
    caps: TransformCapabilities,
    space: GizmoCoordinateSpace,
) -> Option<GizmoHandle> {
    let threshold = pick_threshold(gizmo_scale);
    let handles = active_handles(tool, caps, space);
    let mut best: Option<(f32, GizmoHandle)> = None;

    for handle in handles {
        let Some(dist) =
            handle_pick_distance(ray, anchor, object_rotation, gizmo_scale, handle, space)
        else {
            continue;
        };
        if dist <= threshold && best.map(|(best_d, _)| dist < best_d).unwrap_or(true) {
            best = Some((dist, handle));
        }
    }
    best.map(|(_, handle)| handle)
}

fn handle_pick_distance(
    ray: &Ray3d,
    anchor: Vec3,
    object_rotation: Quat,
    gizmo_scale: f32,
    handle: GizmoHandle,
    space: GizmoCoordinateSpace,
) -> Option<f32> {
    match handle {
        GizmoHandle::TranslateX | GizmoHandle::TranslateY | GizmoHandle::TranslateZ => {
            let axis = oriented_axis(handle.axis()?, object_rotation, space);
            let end = anchor + axis.normalize() * gizmo_scale * GIZMO_HANDLE_LENGTH_FACTOR;
            Some(ray_segment_distance(ray, anchor, end))
        }
        GizmoHandle::TranslateXY | GizmoHandle::TranslateXZ | GizmoHandle::TranslateYZ => {
            let normal = oriented_axis(handle.plane_normal()?, object_rotation, space);
            let hit = ray_plane_intersection(ray, anchor, normal)?;
            let local = hit - anchor;
            let (u_axis, v_axis) = plane_axes(handle)?;
            let u = oriented_axis(u_axis, object_rotation, space);
            let v = oriented_axis(v_axis, object_rotation, space);
            let half = gizmo_scale * 0.22;
            let du = local.dot(u.normalize());
            let dv = local.dot(v.normalize());
            if du.abs() <= half && dv.abs() <= half {
                Some(hit.distance(ray.origin) * 0.001)
            } else {
                None
            }
        }
        GizmoHandle::RotateX | GizmoHandle::RotateY | GizmoHandle::RotateZ => {
            let normal = oriented_axis(handle.plane_normal()?, object_rotation, space);
            let hit = ray_plane_intersection(ray, anchor, normal)?;
            let radial = (hit - anchor).reject_from(normal).length();
            let radius = gizmo_scale * 0.85;
            let thickness = gizmo_scale * 0.08;
            if (radial - radius).abs() <= thickness {
                Some((radial - radius).abs())
            } else {
                None
            }
        }
        GizmoHandle::ScaleX | GizmoHandle::ScaleY | GizmoHandle::ScaleZ => {
            let axis = oriented_axis(handle.axis()?, object_rotation, space);
            let end = anchor + axis.normalize() * gizmo_scale * 0.9;
            Some(ray_segment_distance(ray, anchor, end))
        }
        GizmoHandle::ScaleUniform => {
            let center_dist = point_ray_distance(ray, anchor);
            let radius = gizmo_scale * 0.36;
            if center_dist <= radius {
                Some(center_dist)
            } else {
                None
            }
        }
    }
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
