//! Footprint-wide authoritative terrain sampling for building placement.

use bevy::prelude::*;

use crate::world::occupancy::FootprintShape;
use crate::world::terrain::{TerrainQueryError, try_sample_height_at_position};
use crate::world::{ChunkLayout, WorldData, WorldPosition};

/// One terrain height sample under a building footprint (authoritative simulation Y).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainFootprintSample {
    pub global_xz: Vec2,
    pub height: f32,
}

/// Aggregated terrain statistics for a footprint pose candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainFootprintReport {
    pub samples: Vec<TerrainFootprintSample>,
    pub min_height: f32,
    pub max_height: f32,
    pub height_range: f32,
    /// Least-squares plane `h = a*x + b*z + c` in world space.
    pub plane_a: f32,
    pub plane_b: f32,
    pub plane_c: f32,
    pub plane_normal: Vec3,
    pub plane_slope_degrees: f32,
    pub plane_rms_residual: f32,
    pub plane_peak_residual: f32,
    pub perimeter: Vec<TerrainFootprintSample>,
}

/// Sample authoritative terrain under a rotated footprint at `anchor_global_xz`.
pub fn sample_terrain_under_footprint(
    world: &WorldData,
    layout: ChunkLayout,
    shape: &FootprintShape,
    anchor_global_xz: Vec2,
    yaw_radians: f32,
) -> Result<TerrainFootprintReport, TerrainQueryError> {
    let spacing = footprint_sample_spacing(world, anchor_global_xz, layout);
    let interior = interior_sample_points(shape, anchor_global_xz, yaw_radians, spacing);
    let perimeter = perimeter_sample_points(shape, anchor_global_xz, yaw_radians);

    let mut points: Vec<Vec2> = interior;
    points.extend(perimeter.iter().copied());
    dedupe_xz_points(&mut points);

    let mut samples = Vec::with_capacity(points.len());
    for xz in points {
        let height = sample_height_at_global_xz(world, layout, xz)?;
        samples.push(TerrainFootprintSample {
            global_xz: xz,
            height,
        });
    }

    if samples.is_empty() {
        return Err(TerrainQueryError::InvalidTerrainCoordinate);
    }

    let mut perimeter_samples = Vec::with_capacity(perimeter.len());
    for xz in perimeter {
        if let Some(sample) = samples.iter().find(|s| s.global_xz == xz) {
            perimeter_samples.push(*sample);
        } else {
            let height = sample_height_at_global_xz(world, layout, xz)?;
            perimeter_samples.push(TerrainFootprintSample {
                global_xz: xz,
                height,
            });
        }
    }

    let min_height = samples
        .iter()
        .map(|s| s.height)
        .fold(f32::INFINITY, f32::min);
    let max_height = samples
        .iter()
        .map(|s| s.height)
        .fold(f32::NEG_INFINITY, f32::max);
    let (plane_a, plane_b, plane_c) = fit_plane_least_squares(&samples, anchor_global_xz);
    let plane_normal = plane_normal_from_coefficients(plane_a, plane_b);
    let plane_slope_degrees = plane_normal.y.clamp(-1.0, 1.0).acos().to_degrees();
    let (plane_rms_residual, plane_peak_residual) = plane_residuals(&samples, plane_a, plane_b, plane_c);

    Ok(TerrainFootprintReport {
        samples,
        min_height,
        max_height,
        height_range: max_height - min_height,
        plane_a,
        plane_b,
        plane_c,
        plane_normal,
        plane_slope_degrees,
        plane_rms_residual,
        plane_peak_residual,
        perimeter: perimeter_samples,
    })
}

fn footprint_sample_spacing(
    world: &WorldData,
    anchor_global_xz: Vec2,
    layout: ChunkLayout,
) -> f32 {
    let probe = WorldPosition::from_global(Vec3::new(anchor_global_xz.x, 0.0, anchor_global_xz.y), layout);
    let chunk_id = crate::world::ChunkId::new(probe.chunk);
    if let Some(data) = world.get(chunk_id) {
        let spacing = data.heightfield.spacing_meters();
        if spacing.is_finite() && spacing > 0.0 {
            return spacing * 0.5;
        }
    }
    1.0
}

fn sample_height_at_global_xz(
    world: &WorldData,
    layout: ChunkLayout,
    xz: Vec2,
) -> Result<f32, TerrainQueryError> {
    let position = WorldPosition::from_global(Vec3::new(xz.x, 0.0, xz.y), layout);
    try_sample_height_at_position(world, position)
}

fn interior_sample_points(
    shape: &FootprintShape,
    anchor: Vec2,
    yaw_radians: f32,
    step: f32,
) -> Vec<Vec2> {
    let (min, max) = footprint_aabb(shape, anchor, yaw_radians);
    let step = step.max(0.25);
    let mut points = Vec::new();
    let mut z = min.y;
    while z <= max.y + 1e-4 {
        let mut x = min.x;
        while x <= max.x + 1e-4 {
            let p = Vec2::new(x, z);
            if point_in_footprint(shape, anchor, yaw_radians, p) {
                points.push(p);
            }
            x += step;
        }
        z += step;
    }
    for corner in footprint_corners(shape, anchor, yaw_radians) {
        points.push(corner);
    }
    points
}

fn perimeter_sample_points(shape: &FootprintShape, anchor: Vec2, yaw_radians: f32) -> Vec<Vec2> {
    match shape {
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => rectangle_perimeter(anchor, *width_meters, *depth_meters, yaw_radians, 1.0),
        FootprintShape::Circle { radius_meters } => {
            circle_perimeter(anchor, *radius_meters, 32)
        }
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => ellipse_perimeter(anchor, *radius_x_meters, *radius_z_meters, yaw_radians, 32),
        FootprintShape::BakedCellMask(mask) => {
            baked_mask_perimeter(anchor, mask, yaw_radians)
        }
    }
}

fn rectangle_perimeter(
    anchor: Vec2,
    width: f32,
    depth: f32,
    yaw: f32,
    step: f32,
) -> Vec<Vec2> {
    let half = Vec2::new(width * 0.5, depth * 0.5);
    let corners = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
    ];
    let (sin, cos) = yaw.sin_cos();
    let world_corners: Vec<Vec2> = corners
        .iter()
        .map(|local| {
            Vec2::new(
                local.x * cos - local.y * sin,
                local.x * sin + local.y * cos,
            ) + anchor
        })
        .collect();
    let mut points = Vec::new();
    for i in 0..4 {
        let a = world_corners[i];
        let b = world_corners[(i + 1) % 4];
        let edge = b - a;
        let len = edge.length();
        if len < 1e-4 {
            points.push(a);
            continue;
        }
        let count = ((len / step).ceil() as usize).max(1);
        for j in 0..=count {
            let t = j as f32 / count as f32;
            points.push(a + edge * t);
        }
    }
    points
}

fn circle_perimeter(center: Vec2, radius: f32, segments: usize) -> Vec<Vec2> {
    let mut points = Vec::with_capacity(segments);
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        points.push(center + Vec2::new(cos * radius, sin * radius));
    }
    points
}

fn ellipse_perimeter(
    center: Vec2,
    rx: f32,
    rz: f32,
    yaw: f32,
    segments: usize,
) -> Vec<Vec2> {
    let (sin, cos) = yaw.sin_cos();
    let mut points = Vec::with_capacity(segments);
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sa, ca) = angle.sin_cos();
        let local = Vec2::new(ca * rx, sa * rz);
        points.push(
            Vec2::new(
                local.x * cos - local.y * sin,
                local.x * sin + local.y * cos,
            ) + center,
        );
    }
    points
}

fn baked_mask_perimeter(anchor: Vec2, mask: &crate::world::BakedCellMask, yaw: f32) -> Vec<Vec2> {
    use std::collections::HashSet;

    let cell_size = mask.cell_size_meters;
    let (sin, cos) = yaw.sin_cos();
    let mut blocked = HashSet::new();
    for z in 0..mask.depth_cells {
        for x in 0..mask.width_cells {
            if mask.is_blocked_local(x as i32, z as i32) {
                blocked.insert((x, z));
            }
        }
    }
    let mut points = Vec::new();
    for &(x, z) in &blocked {
        let ox = mask.local_origin.x + x as f32 * cell_size;
        let oz = mask.local_origin.y + z as f32 * cell_size;
        let edges = [
            ((x + 1, z), Vec2::new(ox + cell_size, oz), Vec2::new(ox + cell_size, oz + cell_size)),
            ((x, z + 1), Vec2::new(ox + cell_size, oz + cell_size), Vec2::new(ox, oz + cell_size)),
        ];
        for ((nx, nz), from, to) in edges {
            if nx >= mask.width_cells || nz >= mask.depth_cells || !blocked.contains(&(nx, nz)) {
                let steps = 2;
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    let local = from + (to - from) * t;
                    let world = Vec2::new(
                        local.x * cos - local.y * sin,
                        local.x * sin + local.y * cos,
                    ) + anchor;
                    points.push(world);
                }
            }
        }
    }
    if points.is_empty() {
        points.push(anchor);
    }
    points
}

fn footprint_corners(shape: &FootprintShape, anchor: Vec2, yaw: f32) -> Vec<Vec2> {
    match shape {
        FootprintShape::Rectangle { width_meters, depth_meters } => {
            let half = Vec2::new(width_meters * 0.5, depth_meters * 0.5);
            let locals = [
                Vec2::new(-half.x, -half.y),
                Vec2::new(half.x, -half.y),
                Vec2::new(half.x, half.y),
                Vec2::new(-half.x, half.y),
            ];
            let (sin, cos) = yaw.sin_cos();
            locals
                .iter()
                .map(|local| {
                    Vec2::new(
                        local.x * cos - local.y * sin,
                        local.x * sin + local.y * cos,
                    ) + anchor
                })
                .collect()
        }
        FootprintShape::Circle { radius_meters } => {
            circle_perimeter(anchor, *radius_meters, 8)
        }
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => ellipse_perimeter(anchor, *radius_x_meters, *radius_z_meters, yaw, 8),
        FootprintShape::BakedCellMask(_) => Vec::new(),
    }
}

fn footprint_aabb(shape: &FootprintShape, anchor: Vec2, yaw: f32) -> (Vec2, Vec2) {
    let corners = match shape {
        FootprintShape::Rectangle { .. } => {
            footprint_corners(shape, anchor, yaw)
        }
        FootprintShape::Circle { radius_meters } => circle_perimeter(anchor, *radius_meters, 16),
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => ellipse_perimeter(anchor, *radius_x_meters, *radius_z_meters, yaw, 16),
        FootprintShape::BakedCellMask(mask) => {
            let cell = mask.cell_size_meters;
            let w = mask.width_cells as f32 * cell;
            let d = mask.depth_cells as f32 * cell;
            footprint_corners(
                &FootprintShape::Rectangle {
                    width_meters: w,
                    depth_meters: d,
                },
                anchor + mask.local_origin,
                yaw,
            )
        }
    };
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for p in corners {
        min = min.min(p);
        max = max.max(p);
    }
    (min, max)
}

fn point_in_footprint(shape: &FootprintShape, anchor: Vec2, yaw: f32, point: Vec2) -> bool {
    match shape {
        FootprintShape::Circle { radius_meters } => {
            (point - anchor).length_squared() <= radius_meters * radius_meters
        }
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => {
            let (sin, cos) = yaw.sin_cos();
            let local = point - anchor;
            let lx = local.x * cos + local.y * sin;
            let lz = -local.x * sin + local.y * cos;
            (lx * lx) / (radius_x_meters * radius_x_meters)
                + (lz * lz) / (radius_z_meters * radius_z_meters)
                <= 1.0
        }
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => point_in_oriented_rectangle(point, anchor, *width_meters, *depth_meters, yaw),
        FootprintShape::BakedCellMask(mask) => {
            let (sin, cos) = yaw.sin_cos();
            let local = point - anchor;
            let lx = local.x * cos + local.y * sin;
            let lz = -local.x * sin + local.y * cos;
            let lx = lx - mask.local_origin.x;
            let lz = lz - mask.local_origin.y;
            if lx < 0.0 || lz < 0.0 {
                return false;
            }
            let cx = (lx / mask.cell_size_meters).floor() as i32;
            let cz = (lz / mask.cell_size_meters).floor() as i32;
            mask.is_blocked_local(cx, cz)
        }
    }
}

fn point_in_oriented_rectangle(
    point: Vec2,
    anchor: Vec2,
    width: f32,
    depth: f32,
    yaw: f32,
) -> bool {
    let (sin, cos) = yaw.sin_cos();
    let local = point - anchor;
    let lx = local.x * cos + local.y * sin;
    let lz = -local.x * sin + local.y * cos;
    lx.abs() <= width * 0.5 && lz.abs() <= depth * 0.5
}

fn dedupe_xz_points(points: &mut Vec<Vec2>) {
    points.sort_by(|a, b| {
        a.x
            .total_cmp(&b.x)
            .then_with(|| a.y.total_cmp(&b.y))
    });
    points.dedup_by(|a, b| (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01);
}

/// Fit `h = a*x + b*z + c` in world space using anchor-centered coordinates for stability.
fn fit_plane_least_squares(samples: &[TerrainFootprintSample], anchor_xz: Vec2) -> (f32, f32, f32) {
    let n = samples.len() as f32;
    if n < 1.0 {
        return (0.0, 0.0, 0.0);
    }
    let mut sum_x = 0.0;
    let mut sum_z = 0.0;
    let mut sum_h = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_zz = 0.0;
    let mut sum_xz = 0.0;
    let mut sum_xh = 0.0;
    let mut sum_zh = 0.0;
    for s in samples {
        let x = s.global_xz.x - anchor_xz.x;
        let z = s.global_xz.y - anchor_xz.y;
        let h = s.height;
        sum_x += x;
        sum_z += z;
        sum_h += h;
        sum_xx += x * x;
        sum_zz += z * z;
        sum_xz += x * z;
        sum_xh += x * h;
        sum_zh += z * h;
    }
    let det = sum_xx * (sum_zz * n - sum_z * sum_z) - sum_xz * (sum_xz * n - sum_z * sum_x)
        + sum_x * (sum_xz * sum_z - sum_zz * sum_x);
    if det.abs() < 1e-8 {
        let c = sum_h / n;
        return (0.0, 0.0, c);
    }
    let a = (sum_xh * (sum_zz * n - sum_z * sum_z)
        - sum_xz * (sum_zh * n - sum_z * sum_h)
        + sum_x * (sum_zh * sum_z - sum_zz * sum_xh))
        / det;
    let b = (sum_xx * (sum_zh * n - sum_z * sum_h)
        - sum_xh * (sum_xz * n - sum_z * sum_x)
        + sum_x * (sum_xz * sum_h - sum_zh * sum_x))
        / det;
    let c_local = (sum_xx * (sum_zz * sum_h - sum_z * sum_zh)
        - sum_xz * (sum_xz * sum_h - sum_z * sum_xh)
        + sum_xh * (sum_xz * sum_z - sum_zz * sum_x))
        / det;
    let c = c_local - a * anchor_xz.x - b * anchor_xz.y;
    (a, b, c)
}

/// Plane normal when heightfield samples are vertically exaggerated for presentation.
pub fn presentation_plane_normal(report: &TerrainFootprintReport, vertical_scale: f32) -> Vec3 {
    plane_normal_from_coefficients(report.plane_a * vertical_scale, report.plane_b * vertical_scale)
}

/// Slope angle (degrees from horizontal) for plane coefficients `h = a*x + b*z + c`.
pub fn slope_degrees_from_plane_coefficients(a: f32, b: f32) -> f32 {
    let normal = plane_normal_from_coefficients(a, b);
    normal.y.clamp(-1.0, 1.0).acos().to_degrees()
}

pub fn plane_normal_from_coefficients(a: f32, b: f32) -> Vec3 {
    Vec3::new(-a, 1.0, -b).normalize_or_zero()
}

fn plane_residuals(samples: &[TerrainFootprintSample], a: f32, b: f32, c: f32) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum_sq = 0.0;
    let mut peak: f32 = 0.0;
    for s in samples {
        let predicted = a * s.global_xz.x + b * s.global_xz.y + c;
        let residual = s.height - predicted;
        sum_sq += residual * residual;
        peak = peak.max(residual.abs());
    }
    let rms = (sum_sq / samples.len() as f32).sqrt();
    (rms, peak)
}

pub fn base_height_on_plane_at(
    anchor_xz: Vec2,
    anchor_y: f32,
    up: Vec3,
    sample_xz: Vec2,
) -> f32 {
    let up = up.normalize_or_zero();
    if up.y.abs() < 1e-5 {
        return anchor_y;
    }
    let dx = sample_xz.x - anchor_xz.x;
    let dz = sample_xz.y - anchor_xz.y;
    anchor_y - (dx * up.x + dz * up.z) / up.y
}
