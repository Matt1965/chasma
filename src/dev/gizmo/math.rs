//! Ray and gizmo sizing math (ADR-099).

use bevy::prelude::*;

/// Intersect a ray with a plane defined by point and normal. Returns hit point.
pub fn ray_plane_intersection(ray: &Ray3d, plane_point: Vec3, plane_normal: Vec3) -> Option<Vec3> {
    let normal = plane_normal.normalize_or_zero();
    if normal.length_squared() < 1e-8 {
        return None;
    }
    let denom = ray.direction.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_point - ray.origin).dot(normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ray.origin + ray.direction * t)
}

/// Parameter `t` along the infinite line `line_point + t*line_dir` at the point
/// closest to `ray`. Returns `None` when the line is nearly parallel to the ray
/// (an ill-conditioned configuration, e.g. a vertical axis under a top-down camera).
pub fn ray_axis_closest_param(ray: &Ray3d, line_point: Vec3, line_dir: Vec3) -> Option<f32> {
    let d1 = line_dir.normalize_or_zero();
    if d1.length_squared() < 1e-8 {
        return None;
    }
    let d2 = Vec3::from(ray.direction);
    let b = d1.dot(d2);
    let denom = 1.0 - b * b;
    // `denom` approaches 0 as the axis becomes parallel to the view ray.
    if denom.abs() < 1e-3 {
        return None;
    }
    let r = line_point - ray.origin;
    let d = d1.dot(r);
    let e = d2.dot(r);
    Some((b * e - d) / denom)
}

/// Perpendicular distance from `point` to the (forward) ray.
pub fn point_ray_distance(ray: &Ray3d, point: Vec3) -> f32 {
    let dir = Vec3::from(ray.direction);
    let oc = point - ray.origin;
    let proj = oc.dot(dir).max(0.0);
    let closest = ray.origin + dir * proj;
    point.distance(closest)
}

/// Closest distance between ray and a finite line segment.
pub fn ray_segment_distance(ray: &Ray3d, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-8 {
        return ray.origin.distance(a);
    }
    let t = ((ray.origin - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    let oc = ray.origin - closest;
    let cross = ray.direction.cross(oc);
    cross.length() / ray.direction.length().max(1e-6)
}

/// Signed angle from `from` to `to` around `axis` (radians).
pub fn signed_angle_about_axis(from: Vec3, to: Vec3, axis: Vec3) -> f32 {
    let from_n = from.reject_from(axis).normalize_or_zero();
    let to_n = to.reject_from(axis).normalize_or_zero();
    if from_n.length_squared() < 1e-8 || to_n.length_squared() < 1e-8 {
        return 0.0;
    }
    let sin = axis.normalize().dot(from_n.cross(to_n));
    let cos = from_n.dot(to_n).clamp(-1.0, 1.0);
    sin.atan2(cos)
}

/// Presentation-only gizmo scale for approximately constant screen size.
pub fn apparent_gizmo_scale(
    camera_position: Vec3,
    anchor: Vec3,
    fov_y_radians: f32,
    viewport_height_px: f32,
) -> f32 {
    let distance = camera_position.distance(anchor).max(0.5);
    let desired_px = 130.0;
    let world_per_px = 2.0 * distance * (fov_y_radians * 0.5).tan() / viewport_height_px.max(1.0);
    (desired_px * world_per_px).clamp(0.5, 14.0)
}

/// Handle arrow length as a fraction of presentation scale.
pub const GIZMO_HANDLE_LENGTH_FACTOR: f32 = 1.45;

/// Transform a world axis into local or world space for handle orientation.
pub fn oriented_axis(
    axis: Vec3,
    object_rotation: Quat,
    space: super::tool::GizmoCoordinateSpace,
) -> Vec3 {
    match space {
        super::tool::GizmoCoordinateSpace::World => axis,
        super::tool::GizmoCoordinateSpace::Local => object_rotation * axis,
    }
}

/// Pick threshold scales with gizmo presentation size.
pub fn pick_threshold(gizmo_scale: f32) -> f32 {
    gizmo_scale * 0.16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_horizontal_plane() {
        let ray = Ray3d::new(Vec3::new(0.0, 5.0, 0.0), Dir3::NEG_Y);
        let hit = ray_plane_intersection(&ray, Vec3::ZERO, Vec3::Y).unwrap();
        assert!(hit.y.abs() < 1e-4);
    }

    #[test]
    fn signed_angle_wraps() {
        let a = signed_angle_about_axis(Vec3::X, Vec3::NEG_X, Vec3::Y);
        assert!((a.abs() - std::f32::consts::PI).abs() < 0.01);
    }

    #[test]
    fn axis_param_stable_for_perpendicular_axis() {
        // Camera looking down; dragging along the horizontal X axis is well-conditioned.
        let ray = Ray3d::new(Vec3::new(3.0, 10.0, 0.0), Dir3::NEG_Y);
        let t = ray_axis_closest_param(&ray, Vec3::ZERO, Vec3::X).unwrap();
        assert!((t - 3.0).abs() < 1e-3, "expected ~3.0, got {t}");
    }

    #[test]
    fn axis_param_rejects_parallel_axis() {
        // Vertical axis under a top-down ray is ill-conditioned: must return None
        // rather than an enormous parameter that would teleport the object.
        let ray = Ray3d::new(Vec3::new(0.0, 10.0, 0.0), Dir3::NEG_Y);
        assert!(ray_axis_closest_param(&ray, Vec3::ZERO, Vec3::Y).is_none());
    }
}
