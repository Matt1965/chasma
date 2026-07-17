//! Rotated ellipse collision helpers (ADR-098 DT2).

use bevy::prelude::*;

use super::cell::{OCCUPANCY_CELL_SIZE_METERS, OccupancyCellCoord, circle_intersects_cell};

/// Axis-aligned ellipse in world XZ, rotated by `yaw_radians` about `center`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotatedEllipse {
    pub center: Vec2,
    pub radius_x: f32,
    pub radius_z: f32,
    pub yaw_radians: f32,
}

impl RotatedEllipse {
    pub fn new(center: Vec2, radius_x: f32, radius_z: f32, yaw_radians: f32) -> Self {
        Self {
            center,
            radius_x: radius_x.max(0.0),
            radius_z: radius_z.max(0.0),
            yaw_radians,
        }
    }

    pub fn is_degenerate(self) -> bool {
        self.radius_x <= 0.0 || self.radius_z <= 0.0
    }

    /// World-space AABB of the rotated ellipse.
    pub fn world_aabb(self) -> (Vec2, Vec2) {
        if self.is_degenerate() {
            return (self.center, self.center);
        }
        let (sin, cos) = self.yaw_radians.sin_cos();
        let rx_cos = self.radius_x * cos.abs();
        let rx_sin = self.radius_x * sin.abs();
        let rz_cos = self.radius_z * cos.abs();
        let rz_sin = self.radius_z * sin.abs();
        let extent_x = rx_cos + rz_sin;
        let extent_z = rx_sin + rz_cos;
        let min = self.center - Vec2::new(extent_x, extent_z);
        let max = self.center + Vec2::new(extent_x, extent_z);
        (min, max)
    }
}

/// Transform a world XZ point into ellipse-local coordinates (inverse yaw).
pub fn world_to_ellipse_local(point: Vec2, center: Vec2, yaw_radians: f32) -> Vec2 {
    let delta = point - center;
    let (sin, cos) = yaw_radians.sin_cos();
    Vec2::new(
        delta.x * cos + delta.y * sin,
        -delta.x * sin + delta.y * cos,
    )
}

/// Whether `point` lies inside the normalized ellipse (inclusive boundary).
pub fn ellipse_contains_point(ellipse: RotatedEllipse, point: Vec2) -> bool {
    if ellipse.is_degenerate() {
        return false;
    }
    let local = world_to_ellipse_local(point, ellipse.center, ellipse.yaw_radians);
    let nx = local.x / ellipse.radius_x;
    let nz = local.y / ellipse.radius_z;
    nx * nx + nz * nz <= 1.0 + 1e-5
}

/// Closest point on the axis-aligned ellipse (ellipse-local space) to `local_point`.
pub fn closest_point_on_axis_aligned_ellipse(
    local_point: Vec2,
    radius_x: f32,
    radius_z: f32,
) -> Vec2 {
    if radius_x <= 0.0 || radius_z <= 0.0 {
        return Vec2::ZERO;
    }
    let mut px = local_point.x;
    let mut pz = local_point.y;
    if px.abs() < 1e-8 && pz.abs() < 1e-8 {
        return Vec2::new(radius_x, 0.0);
    }
    for _ in 0..16 {
        let denom_x = radius_x * radius_x;
        let denom_z = radius_z * radius_z;
        let t = (px * px) / denom_x + (pz * pz) / denom_z;
        if t <= 1.0 {
            break;
        }
        let scale = t.sqrt();
        px /= scale;
        pz /= scale;
    }
    Vec2::new(px, pz)
}

/// Whether a unit circle overlaps a rotated ellipse (broad-phase + closest-point).
pub fn circle_overlaps_rotated_ellipse(
    circle_center: Vec2,
    circle_radius: f32,
    ellipse: RotatedEllipse,
) -> bool {
    if circle_radius <= 0.0 {
        return ellipse_contains_point(ellipse, circle_center);
    }
    if ellipse.is_degenerate() {
        return false;
    }
    let local = world_to_ellipse_local(circle_center, ellipse.center, ellipse.yaw_radians);
    if local.length_squared() <= 1e-10 {
        return true;
    }
    let closest_local =
        closest_point_on_axis_aligned_ellipse(local, ellipse.radius_x, ellipse.radius_z);
    let closest_world = ellipse.center
        + Vec2::new(
            closest_local.x * ellipse.yaw_radians.cos()
                - closest_local.y * ellipse.yaw_radians.sin(),
            closest_local.x * ellipse.yaw_radians.sin()
                + closest_local.y * ellipse.yaw_radians.cos(),
        );
    closest_world.distance(circle_center) <= circle_radius + 1e-4
}

/// Conservative ellipse-vs-cell overlap for occupancy rasterization.
pub fn ellipse_overlaps_cell(ellipse: RotatedEllipse, cell: OccupancyCellCoord) -> bool {
    if ellipse.is_degenerate() {
        return false;
    }
    let (min, max) = cell.bounds_global();
    let (emin, emax) = ellipse.world_aabb();
    if emax.x < min.x || emin.x > max.x || emax.y < min.y || emin.y > max.y {
        return false;
    }
    let corners = [
        Vec2::new(min.x, min.y),
        Vec2::new(max.x, min.y),
        Vec2::new(max.x, max.y),
        Vec2::new(min.x, max.y),
    ];
    if corners.iter().any(|c| ellipse_contains_point(ellipse, *c)) {
        return true;
    }
    if ellipse_contains_point(ellipse, cell.center_global()) {
        return true;
    }
    circle_overlaps_rotated_ellipse(
        cell.center_global(),
        OCCUPANCY_CELL_SIZE_METERS * 0.75,
        ellipse,
    )
}

/// Deterministic sorted occupancy cells for a rotated ellipse.
pub fn cells_for_rotated_ellipse(ellipse: RotatedEllipse) -> Vec<OccupancyCellCoord> {
    if ellipse.is_degenerate() {
        return Vec::new();
    }
    let (min, max) = ellipse.world_aabb();
    let size = OCCUPANCY_CELL_SIZE_METERS;
    let min_x = (min.x / size).floor() as i32;
    let max_x = (max.x / size).floor() as i32;
    let min_z = (min.y / size).floor() as i32;
    let max_z = (max.y / size).floor() as i32;
    let mut cells = Vec::new();
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            let cell = OccupancyCellCoord::new(x, z);
            if ellipse_overlaps_cell(ellipse, cell) {
                cells.push(cell);
            }
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

/// Circle cells helper delegating to ellipse when radii differ.
pub fn cells_for_circle_or_ellipse(
    center: Vec2,
    radius_x: f32,
    radius_z: f32,
    yaw_radians: f32,
) -> Vec<OccupancyCellCoord> {
    if (radius_x - radius_z).abs() < 1e-4 {
        cells_for_uniform_circle(center, radius_x)
    } else {
        cells_for_rotated_ellipse(RotatedEllipse::new(center, radius_x, radius_z, yaw_radians))
    }
}

fn cells_for_uniform_circle(center: Vec2, radius: f32) -> Vec<OccupancyCellCoord> {
    if radius <= 0.0 {
        return Vec::new();
    }
    let size = OCCUPANCY_CELL_SIZE_METERS;
    let min_x = ((center.x - radius) / size).floor() as i32;
    let max_x = ((center.x + radius) / size).floor() as i32;
    let min_z = ((center.y - radius) / size).floor() as i32;
    let max_z = ((center.y + radius) / size).floor() as i32;
    let mut cells = Vec::new();
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            let cell = OccupancyCellCoord::new(x, z);
            if circle_intersects_cell(center, radius, cell) {
                cells.push(cell);
            }
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_inside_outside_axis_aligned() {
        let e = RotatedEllipse::new(Vec2::ZERO, 2.0, 1.0, 0.0);
        assert!(ellipse_contains_point(e, Vec2::new(1.0, 0.0)));
        assert!(ellipse_contains_point(e, Vec2::new(0.0, 0.5)));
        assert!(!ellipse_contains_point(e, Vec2::new(2.1, 0.0)));
    }

    #[test]
    fn rotated_ellipse_contains_rotated_point() {
        let e = RotatedEllipse::new(Vec2::ZERO, 3.0, 1.0, std::f32::consts::FRAC_PI_4);
        assert!(ellipse_contains_point(e, Vec2::new(1.0, 1.0)));
    }

    #[test]
    fn unit_circle_overlaps_major_axis() {
        let e = RotatedEllipse::new(Vec2::ZERO, 4.0, 1.0, 0.0);
        assert!(circle_overlaps_rotated_ellipse(Vec2::new(4.5, 0.0), 0.6, e));
    }

    #[test]
    fn unit_circle_misses_minor_axis_gap() {
        let e = RotatedEllipse::new(Vec2::ZERO, 4.0, 0.5, 0.0);
        assert!(!circle_overlaps_rotated_ellipse(
            Vec2::new(0.0, 2.0),
            0.4,
            e
        ));
    }

    #[test]
    fn cells_are_sorted_and_unique() {
        let cells =
            cells_for_rotated_ellipse(RotatedEllipse::new(Vec2::new(5.0, 5.0), 2.0, 1.0, 0.3));
        assert!(!cells.is_empty());
        for window in cells.windows(2) {
            assert!(window[0] <= window[1]);
            assert_ne!(window[0], window[1]);
        }
    }

    #[test]
    fn equal_radii_use_circle_path() {
        let cells = cells_for_circle_or_ellipse(Vec2::ZERO, 2.0, 2.0, 0.5);
        assert!(!cells.is_empty());
    }
}
