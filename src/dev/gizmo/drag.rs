//! Transform drag math for gizmo handles (ADR-099).

use bevy::prelude::*;

use crate::world::{AuthoringScale, ChunkLayout, QuantizedOrientation, WorldPosition};

use super::handles::GizmoHandle;
use super::math::{
    oriented_axis, ray_axis_closest_param, ray_plane_intersection, signed_angle_about_axis,
};
use super::snap::TransformSnapSettings;
use super::state::{DoodadPreviewPlacement, GizmoAxisConstraint};
use super::tool::GizmoCoordinateSpace;

/// Maximum translation magnitude accepted from a single drag update (meters).
/// Guards against ill-conditioned ray/plane intersections teleporting objects.
const MAX_TRANSLATE_DELTA_METERS: f32 = 4096.0;

/// Apply a drag from `start_ray` to `current_ray` for the active handle (render-space anchor).
///
/// `vertical_scale` is the terrain render Y exaggeration used to build `anchor_render`.
/// Translation results are computed in render space and converted back to authoritative
/// world space by dividing Y by this factor (X/Z are never scaled).
pub fn apply_drag(
    handle: GizmoHandle,
    start_ray: &Ray3d,
    current_ray: &Ray3d,
    start: DoodadPreviewPlacement,
    anchor_render: Vec3,
    layout: ChunkLayout,
    vertical_scale: f32,
    object_rotation: Quat,
    space: GizmoCoordinateSpace,
    snap: TransformSnapSettings,
    finer_snap: bool,
    axis_constraint: Option<GizmoAxisConstraint>,
    min_scale: f32,
    max_scale: f32,
) -> Option<DoodadPreviewPlacement> {
    match handle {
        GizmoHandle::TranslateX
        | GizmoHandle::TranslateY
        | GizmoHandle::TranslateZ
        | GizmoHandle::TranslateXY
        | GizmoHandle::TranslateXZ
        | GizmoHandle::TranslateYZ => apply_translate_drag_global(
            handle,
            start_ray,
            current_ray,
            anchor_render,
            start,
            layout,
            vertical_scale,
            object_rotation,
            space,
            snap,
            finer_snap,
            axis_constraint,
        ),
        GizmoHandle::RotateX | GizmoHandle::RotateY | GizmoHandle::RotateZ => apply_rotate_drag(
            handle,
            start_ray,
            current_ray,
            start,
            anchor_render,
            object_rotation,
            space,
            snap,
            finer_snap,
        ),
        GizmoHandle::ScaleX
        | GizmoHandle::ScaleY
        | GizmoHandle::ScaleZ
        | GizmoHandle::ScaleUniform => apply_scale_drag(
            handle,
            start_ray,
            current_ray,
            start,
            anchor_render,
            object_rotation,
            snap,
            finer_snap,
            min_scale,
            max_scale,
        ),
    }
}

fn apply_translate_drag_global(
    handle: GizmoHandle,
    start_ray: &Ray3d,
    current_ray: &Ray3d,
    start_global: Vec3,
    start: DoodadPreviewPlacement,
    layout: ChunkLayout,
    vertical_scale: f32,
    object_rotation: Quat,
    space: GizmoCoordinateSpace,
    snap: TransformSnapSettings,
    finer_snap: bool,
    axis_constraint: Option<GizmoAxisConstraint>,
) -> Option<DoodadPreviewPlacement> {
    let mut delta = match single_translate_axis(handle, axis_constraint) {
        // Single-axis: use ray-vs-axis closest point (stable at grazing angles).
        Some(axis_local) => {
            let axis = oriented_axis(axis_local, object_rotation, space).normalize_or_zero();
            let t_start = ray_axis_closest_param(start_ray, start_global, axis)?;
            let t_current = ray_axis_closest_param(current_ray, start_global, axis)?;
            axis * (t_current - t_start)
        }
        // Plane handles: intersect against the plane and mask to its axes.
        None => {
            let (plane_normal, mask) = translate_plane(handle, axis_constraint)?;
            let normal = oriented_axis(plane_normal, object_rotation, space);
            let start_hit = ray_plane_intersection(start_ray, start_global, normal)?;
            let current_hit = ray_plane_intersection(current_ray, start_global, normal)?;
            let raw = current_hit - start_hit;
            Vec3::new(raw.x * mask.x, raw.y * mask.y, raw.z * mask.z)
        }
    };

    if !delta.is_finite() || delta.length() > MAX_TRANSLATE_DELTA_METERS {
        return None;
    }

    delta.x = snap.snap_translation(delta.x, finer_snap);
    delta.y = snap.snap_translation(delta.y, finer_snap);
    delta.z = snap.snap_translation(delta.z, finer_snap);
    let new_render_global = start_global + delta;
    if !new_render_global.is_finite() {
        return None;
    }
    // Convert the render-space result back to authoritative world space. Terrain render
    // scales only Y (see `world_position_to_render_global`), so undo that here to avoid
    // Y compounding across drag frames.
    let vs = if vertical_scale.abs() > 1e-6 {
        vertical_scale
    } else {
        1.0
    };
    let world_global = Vec3::new(
        new_render_global.x,
        new_render_global.y / vs,
        new_render_global.z,
    );
    let position = WorldPosition::from_global(world_global, layout);
    Some(DoodadPreviewPlacement {
        position,
        orientation: start.orientation,
        scale: start.scale,
    })
}

/// Local-space axis for a single-axis translate handle (or constraint), else `None`.
fn single_translate_axis(
    handle: GizmoHandle,
    axis_constraint: Option<GizmoAxisConstraint>,
) -> Option<Vec3> {
    if let Some(axis) = axis_constraint {
        return Some(match axis {
            GizmoAxisConstraint::X => Vec3::X,
            GizmoAxisConstraint::Y => Vec3::Y,
            GizmoAxisConstraint::Z => Vec3::Z,
        });
    }
    match handle {
        GizmoHandle::TranslateX => Some(Vec3::X),
        GizmoHandle::TranslateY => Some(Vec3::Y),
        GizmoHandle::TranslateZ => Some(Vec3::Z),
        _ => None,
    }
}

fn translate_plane(
    handle: GizmoHandle,
    axis_constraint: Option<GizmoAxisConstraint>,
) -> Option<(Vec3, Vec3)> {
    if let Some(axis) = axis_constraint {
        return match axis {
            GizmoAxisConstraint::X => Some((Vec3::Y, Vec3::new(1.0, 0.0, 0.0))),
            GizmoAxisConstraint::Y => Some((Vec3::X, Vec3::new(0.0, 1.0, 0.0))),
            GizmoAxisConstraint::Z => Some((Vec3::Y, Vec3::new(0.0, 0.0, 1.0))),
        };
    }
    match handle {
        GizmoHandle::TranslateX => Some((Vec3::Y, Vec3::new(1.0, 0.0, 0.0))),
        GizmoHandle::TranslateY => Some((Vec3::X, Vec3::new(0.0, 1.0, 0.0))),
        GizmoHandle::TranslateZ => Some((Vec3::Y, Vec3::new(0.0, 0.0, 1.0))),
        GizmoHandle::TranslateXY => Some((Vec3::Z, Vec3::ONE)),
        GizmoHandle::TranslateXZ => Some((Vec3::Y, Vec3::new(1.0, 0.0, 1.0))),
        GizmoHandle::TranslateYZ => Some((Vec3::X, Vec3::new(0.0, 1.0, 1.0))),
        _ => None,
    }
}

fn apply_rotate_drag(
    handle: GizmoHandle,
    start_ray: &Ray3d,
    current_ray: &Ray3d,
    start: DoodadPreviewPlacement,
    anchor: Vec3,
    object_rotation: Quat,
    space: GizmoCoordinateSpace,
    snap: TransformSnapSettings,
    finer_snap: bool,
) -> Option<DoodadPreviewPlacement> {
    let axis = oriented_axis(handle.axis()?, object_rotation, space);
    let start_hit = ray_plane_intersection(start_ray, anchor, axis)?;
    let current_hit = ray_plane_intersection(current_ray, anchor, axis)?;
    let from = (start_hit - anchor).reject_from(axis);
    let to = (current_hit - anchor).reject_from(axis);
    let mut delta_rad = signed_angle_about_axis(from, to, axis);
    if snap.rotation_enabled || finer_snap {
        let delta_deg = snap.snap_rotation_degrees(delta_rad.to_degrees(), finer_snap);
        delta_rad = delta_deg.to_radians();
    }
    let start_quat = start.orientation.to_quat();
    let delta_quat = Quat::from_axis_angle(axis.normalize(), delta_rad);
    let new_quat = match space {
        GizmoCoordinateSpace::World => delta_quat * start_quat,
        GizmoCoordinateSpace::Local => start_quat * delta_quat,
    };
    let orientation = QuantizedOrientation::from_quat(new_quat).ok()?;
    Some(DoodadPreviewPlacement {
        position: start.position,
        orientation,
        scale: start.scale,
    })
}

fn apply_scale_drag(
    handle: GizmoHandle,
    start_ray: &Ray3d,
    current_ray: &Ray3d,
    start: DoodadPreviewPlacement,
    anchor: Vec3,
    object_rotation: Quat,
    snap: TransformSnapSettings,
    finer_snap: bool,
    min_scale: f32,
    max_scale: f32,
) -> Option<DoodadPreviewPlacement> {
    let start_scale = start.scale_vec3();
    let plane_normal = match handle {
        GizmoHandle::ScaleUniform => (current_ray.origin - anchor).normalize_or_zero(),
        GizmoHandle::ScaleX => object_rotation * Vec3::X,
        GizmoHandle::ScaleY => object_rotation * Vec3::Y,
        GizmoHandle::ScaleZ => object_rotation * Vec3::Z,
        _ => return None,
    };
    let start_hit = ray_plane_intersection(start_ray, anchor, plane_normal)?;
    let current_hit = ray_plane_intersection(current_ray, anchor, plane_normal)?;
    let signed_delta = (current_hit - start_hit).dot(plane_normal);
    let sensitivity = 0.01;
    let mut new_scale = start_scale;
    match handle {
        GizmoHandle::ScaleUniform => {
            let factor = 1.0 + signed_delta * sensitivity;
            new_scale *= factor;
        }
        GizmoHandle::ScaleX => new_scale.x = start_scale.x + signed_delta * sensitivity,
        GizmoHandle::ScaleY => new_scale.y = start_scale.y + signed_delta * sensitivity,
        GizmoHandle::ScaleZ => new_scale.z = start_scale.z + signed_delta * sensitivity,
        _ => {}
    }

    new_scale.x = snap
        .snap_scale(new_scale.x.clamp(min_scale, max_scale), finer_snap)
        .max(min_scale);
    new_scale.y = snap
        .snap_scale(new_scale.y.clamp(min_scale, max_scale), finer_snap)
        .max(min_scale);
    new_scale.z = snap
        .snap_scale(new_scale.z.clamp(min_scale, max_scale), finer_snap)
        .max(min_scale);

    let scale = AuthoringScale::from_non_uniform_f32(new_scale.x, new_scale.y, new_scale.z).ok()?;
    Some(DoodadPreviewPlacement {
        position: start.position,
        orientation: start.orientation,
        scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, DoodadPlacement, LocalPosition, WorldPosition};

    fn start_placement() -> DoodadPreviewPlacement {
        DoodadPreviewPlacement::from_placement(DoodadPlacement::identity_at(WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::ZERO),
        )))
    }

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    #[test]
    fn topdown_vertical_translate_does_not_teleport() {
        // Top-down camera dragging the vertical (Y) handle used to intersect an
        // edge-on plane and fling the object thousands of meters away. The
        // ray-vs-axis solver rejects this ill-conditioned drag instead.
        let start_ray = Ray3d::new(Vec3::new(0.0, 50.0, 0.0), Dir3::NEG_Y);
        let current_ray = Ray3d::new(Vec3::new(0.5, 50.0, 0.5), Dir3::NEG_Y);
        let result = apply_drag(
            GizmoHandle::TranslateY,
            &start_ray,
            &current_ray,
            start_placement(),
            Vec3::ZERO,
            layout(),
            1.0,
            Quat::IDENTITY,
            GizmoCoordinateSpace::World,
            TransformSnapSettings::default(),
            false,
            None,
            0.05,
            20.0,
        );
        assert!(
            result.is_none(),
            "vertical drag under top-down view must be rejected"
        );
    }

    #[test]
    fn horizontal_translate_moves_reasonably() {
        // Camera above and to the side; dragging along X should move a bounded amount.
        let start_ray = Ray3d::new(
            Vec3::new(0.0, 40.0, 40.0),
            Dir3::new_unchecked(Vec3::new(0.0, -0.7071, -0.7071)),
        );
        let current_ray = Ray3d::new(
            Vec3::new(5.0, 40.0, 40.0),
            Dir3::new_unchecked(Vec3::new(0.0, -0.7071, -0.7071)),
        );
        let result = apply_drag(
            GizmoHandle::TranslateX,
            &start_ray,
            &current_ray,
            start_placement(),
            Vec3::ZERO,
            layout(),
            1.0,
            Quat::IDENTITY,
            GizmoCoordinateSpace::World,
            TransformSnapSettings::default(),
            false,
            None,
            0.05,
            20.0,
        )
        .expect("horizontal drag should produce a placement");
        let moved = result.position.to_global(layout());
        assert!(
            moved.length() <= MAX_TRANSLATE_DELTA_METERS,
            "translation must stay bounded, got {moved:?}"
        );
    }

    #[test]
    fn vertical_scale_does_not_compound_y() {
        // Anchor is render-space (Y already multiplied by vertical_scale). A tiny drag
        // must not re-multiply Y — the stored world Y should be anchor.y / vertical_scale
        // plus the small world-space delta, never a growing multiple.
        let vertical_scale = 4.0;
        let world_y = 10.0_f32;
        let anchor_render = Vec3::new(0.0, world_y * vertical_scale, 0.0);
        // Side camera so the Y axis is well-conditioned.
        let dir = Dir3::new_unchecked(Vec3::new(0.0, -0.7071, -0.7071));
        let start_ray = Ray3d::new(Vec3::new(0.0, 80.0, 80.0), dir);
        let current_ray = Ray3d::new(Vec3::new(0.0, 80.5, 80.0), dir);
        let result = apply_drag(
            GizmoHandle::TranslateY,
            &start_ray,
            &current_ray,
            start_placement(),
            anchor_render,
            layout(),
            vertical_scale,
            Quat::IDENTITY,
            GizmoCoordinateSpace::World,
            TransformSnapSettings::default(),
            false,
            None,
            0.05,
            20.0,
        )
        .expect("vertical drag should produce a placement");
        let stored_world_y = result.position.to_global(layout()).y;
        // Must stay near the original world Y (10), not explode toward world_y * vs (40+).
        assert!(
            (stored_world_y - world_y).abs() < 5.0,
            "world Y must not compound with vertical_scale, got {stored_world_y}"
        );
    }
}
