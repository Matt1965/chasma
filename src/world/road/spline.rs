use bevy::prelude::*;

use super::control_point::{CornerMode, RoadControlPoint};
use super::road::Road;

const CENTRIPETAL_ALPHA: f32 = 0.5;
const MIN_KNOT_DELTA: f32 = 1e-4;

/// One derived spline sample in simulation XZ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadSplineSample {
    pub position: Vec2,
    pub tangent: Vec2,
    pub distance_m: f32,
}

/// Sample a road spline at a normalized parameter in `[0, 1]`.
pub fn sample_road_spline_at_t(road: &Road, t: f32) -> Option<RoadSplineSample> {
    if road.control_points.len() < 2 {
        return None;
    }
    let points = road.control_points.iter().map(|p| p.xz()).collect::<Vec<_>>();
    let total_length = approximate_road_length(&points);
    if total_length <= 0.0 {
        return Some(RoadSplineSample {
            position: points[0],
            tangent: Vec2::X,
            distance_m: 0.0,
        });
    }
    let target_distance = t.clamp(0.0, 1.0) * total_length;
    sample_road_spline_at_distance(road, target_distance)
}

/// Sample a road spline at an arc-length distance in meters from the start.
pub fn sample_road_spline_at_distance(road: &Road, distance_m: f32) -> Option<RoadSplineSample> {
    if road.control_points.len() < 2 {
        return None;
    }
    let points = road.control_points.iter().map(|p| p.xz()).collect::<Vec<_>>();
    let total_length = approximate_road_length(&points);
    if total_length <= 0.0 {
        return Some(RoadSplineSample {
            position: points[0],
            tangent: Vec2::X,
            distance_m: 0.0,
        });
    }

    let target = distance_m.clamp(0.0, total_length);
    let mut accumulated = 0.0;
    let segment_count = points.len() - 1;
    for segment_index in 0..segment_count {
        let segment_length = segment_arc_length(&points, segment_index);
        if accumulated + segment_length >= target || segment_index == segment_count - 1 {
            let local_distance = target - accumulated;
            let local_t = if segment_length > 0.0 {
                (local_distance / segment_length).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let position = sample_segment(&points, &road.control_points, segment_index, local_t);
            let tangent = sample_segment_tangent(&points, &road.control_points, segment_index, local_t);
            return Some(RoadSplineSample {
                position,
                tangent: tangent.normalize_or_zero(),
                distance_m: target,
            });
        }
        accumulated += segment_length;
    }

    let last = points[points.len() - 1];
    Some(RoadSplineSample {
        position: last,
        tangent: (last - points[points.len() - 2]).normalize_or_zero(),
        distance_m: total_length,
    })
}

/// Derive a deterministic polyline along the road at a fixed spacing.
pub fn sample_road_polyline(road: &Road, spacing_m: f32) -> Vec<RoadSplineSample> {
    if road.control_points.len() < 2 || spacing_m <= 0.0 || !spacing_m.is_finite() {
        return Vec::new();
    }
    let points = road.control_points.iter().map(|p| p.xz()).collect::<Vec<_>>();
    let total_length = approximate_road_length(&points);
    if total_length <= 0.0 {
        return vec![RoadSplineSample {
            position: points[0],
            tangent: Vec2::X,
            distance_m: 0.0,
        }];
    }

    let sample_count = ((total_length / spacing_m).ceil() as usize).max(1);
    (0..=sample_count)
        .map(|index| {
            let t = index as f32 / sample_count as f32;
            sample_road_spline_at_distance(road, t * total_length).expect("sample within length")
        })
        .collect()
}

fn approximate_road_length(points: &[Vec2]) -> f32 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    for segment_index in 0..points.len() - 1 {
        total += segment_arc_length(points, segment_index);
    }
    total
}

fn segment_arc_length(points: &[Vec2], segment_index: usize) -> f32 {
    if points.len() < 2 {
        return 0.0;
    }
    if points.len() == 2 {
        return points[1].distance(points[0]);
    }
    let samples = 16;
    let mut length = 0.0;
    let mut previous = sample_segment(points, &[], segment_index, 0.0);
    for sample_index in 1..=samples {
        let t = sample_index as f32 / samples as f32;
        let current = sample_segment(points, &[], segment_index, t);
        length += current.distance(previous);
        previous = current;
    }
    length
}

fn sample_segment(
    points: &[Vec2],
    control_points: &[RoadControlPoint],
    segment_index: usize,
    local_t: f32,
) -> Vec2 {
    if points.len() == 2 {
        return points[0].lerp(points[1], local_t);
    }

    let p0 = endpoint_for_segment(points, control_points, segment_index, -1);
    let p1 = points[segment_index];
    let p2 = points[segment_index + 1];
    let p3 = endpoint_for_segment(points, control_points, segment_index, 2);

    if control_points
        .get(segment_index + 1)
        .is_some_and(|point| point.corner_mode == CornerMode::Sharp)
    {
        return p1.lerp(p2, local_t);
    }

    centripetal_catmull_rom(p0, p1, p2, p3, local_t)
}

fn sample_segment_tangent(
    points: &[Vec2],
    control_points: &[RoadControlPoint],
    segment_index: usize,
    local_t: f32,
) -> Vec2 {
    let epsilon = 1e-3;
    let t0 = (local_t - epsilon).max(0.0);
    let t1 = (local_t + epsilon).min(1.0);
    let p0 = sample_segment(points, control_points, segment_index, t0);
    let p1 = sample_segment(points, control_points, segment_index, t1);
    p1 - p0
}

fn endpoint_for_segment(
    points: &[Vec2],
    control_points: &[RoadControlPoint],
    segment_index: usize,
    endpoint_offset: isize,
) -> Vec2 {
    let target_index = segment_index as isize + endpoint_offset;
    if target_index < 0 {
        let p0 = points[0];
        let p1 = points[1];
        if control_points
            .first()
            .is_some_and(|point| point.corner_mode == CornerMode::Sharp)
        {
            return p0;
        }
        return p0 + (p0 - p1);
    }
    let last_index = points.len() - 1;
    if target_index as usize > last_index {
        let p0 = points[last_index - 1];
        let p1 = points[last_index];
        if control_points
            .last()
            .is_some_and(|point| point.corner_mode == CornerMode::Sharp)
        {
            return p1;
        }
        return p1 + (p1 - p0);
    }
    points[target_index as usize]
}

fn centripetal_catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t0 = 0.0;
    let t1 = knot_delta(t0, p0, p1);
    let t2 = knot_delta(t1, p1, p2);
    let t3 = knot_delta(t2, p2, p3);

    if (t2 - t1).abs() <= MIN_KNOT_DELTA {
        return p1.lerp(p2, t);
    }

    let u = t1 + (t2 - t1) * t.clamp(0.0, 1.0);
    let a1 = lerp_by_knot(p0, p1, t0, t1, u);
    let a2 = lerp_by_knot(p1, p2, t1, t2, u);
    let a3 = lerp_by_knot(p2, p3, t2, t3, u);
    let b1 = lerp_by_knot(a1, a2, t0, t2, u);
    let b2 = lerp_by_knot(a2, a3, t1, t3, u);
    lerp_by_knot(b1, b2, t1, t2, u)
}

fn knot_delta(previous: f32, from: Vec2, to: Vec2) -> f32 {
    let distance = from.distance(to).max(MIN_KNOT_DELTA);
    previous + distance.powf(CENTRIPETAL_ALPHA)
}

fn lerp_by_knot(a: Vec2, b: Vec2, ta: f32, tb: f32, t: f32) -> Vec2 {
    if (tb - ta).abs() <= MIN_KNOT_DELTA {
        return a;
    }
    let weight = (t - ta) / (tb - ta);
    a.lerp(b, weight)
}
