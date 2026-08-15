//! Shared authored opening interval geometry for Interior clearance and segment crossing (NAV-OPENING-1).

use bevy::prelude::*;

use super::ENTRANCE_BOUNDARY_TOLERANCE;
use crate::world::space::{PortalRecord, PortalType, SpaceRegistry};
use crate::world::{BuildingId, SpaceId};

/// Parametric interval along a polygon edge from vertex `a` toward `b` (`t` in `[0, 1]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeParametricInterval {
    pub t_start: f32,
    pub t_end: f32,
}

impl EdgeParametricInterval {
    pub fn new(t_start: f32, t_end: f32) -> Self {
        Self {
            t_start: t_start.min(t_end),
            t_end: t_start.max(t_end),
        }
    }

    pub fn is_empty(self) -> bool {
        self.t_end <= self.t_start + f32::EPSILON
    }
}

/// Project `threshold` onto edge `a→b`, returning parametric `t` when on-edge within tolerance.
pub fn threshold_parametric_on_edge(a: Vec2, b: Vec2, threshold: Vec2) -> Option<f32> {
    let edge = b - a;
    let len_sq = edge.length_squared();
    if len_sq <= f32::EPSILON {
        return None;
    }
    let t = ((threshold - a).dot(edge) / len_sq).clamp(0.0, 1.0);
    let on_edge = a + edge * t;
    if on_edge.distance(threshold) > ENTRANCE_BOUNDARY_TOLERANCE {
        return None;
    }
    Some(t)
}

/// Full authored opening interval along an edge (agent center not yet shrunk).
pub fn authored_opening_interval_on_edge(
    a: Vec2,
    b: Vec2,
    threshold: Vec2,
    opening_half_width: f32,
) -> Option<EdgeParametricInterval> {
    if !(opening_half_width > 0.0) {
        return None;
    }
    let edge_len = (b - a).length();
    if edge_len <= f32::EPSILON {
        return None;
    }
    let Some(t_thresh) = threshold_parametric_on_edge(a, b, threshold) else {
        return None;
    };
    let half_t = opening_half_width / edge_len;
    let t_start = (t_thresh - half_t).clamp(0.0, 1.0);
    let t_end = (t_thresh + half_t).clamp(0.0, 1.0);
    let interval = EdgeParametricInterval::new(t_start, t_end);
    if interval.is_empty() {
        None
    } else {
        Some(interval)
    }
}

/// Usable interval for an agent center: authored opening shrunk by radius at both endpoints.
pub fn usable_center_opening_interval_on_edge(
    a: Vec2,
    b: Vec2,
    threshold: Vec2,
    opening_half_width: f32,
    agent_radius: f32,
) -> Option<EdgeParametricInterval> {
    if agent_radius <= 0.0 || !agent_radius.is_finite() {
        return authored_opening_interval_on_edge(a, b, threshold, opening_half_width);
    }
    let usable_half_width = opening_half_width - agent_radius;
    if usable_half_width <= f32::EPSILON {
        return None;
    }
    authored_opening_interval_on_edge(a, b, threshold, usable_half_width)
}

/// Whether `point` lies within the agent-usable center opening interval on edge `a→b`.
pub fn point_within_usable_center_opening_on_edge(
    point: Vec2,
    a: Vec2,
    b: Vec2,
    threshold: Vec2,
    opening_half_width: f32,
    agent_radius: f32,
) -> bool {
    let Some(interval) =
        usable_center_opening_interval_on_edge(a, b, threshold, opening_half_width, agent_radius)
    else {
        return false;
    };
    point_parametric_on_edge(point, a, b).is_some_and(|t| interval.contains_t(t))
}

/// Whether `point` lies within any merged agent-usable interval on edge `a→b`.
pub fn point_within_merged_usable_intervals_on_edge(
    point: Vec2,
    a: Vec2,
    b: Vec2,
    intervals: &[EdgeParametricInterval],
) -> bool {
    let Some(t) = point_parametric_on_edge(point, a, b) else {
        return false;
    };
    intervals.iter().any(|interval| interval.contains_t(t))
}

/// Whether `point` lies within the authored opening interval on edge `a→b`.
pub fn point_within_authored_opening_on_edge(
    point: Vec2,
    a: Vec2,
    b: Vec2,
    threshold: Vec2,
    opening_half_width: f32,
) -> bool {
    let Some(interval) = authored_opening_interval_on_edge(a, b, threshold, opening_half_width)
    else {
        return false;
    };
    point_parametric_on_edge(point, a, b).is_some_and(|t| interval.contains_t(t))
}

/// Merge overlapping parametric intervals on the same edge.
pub fn merge_edge_intervals(
    mut intervals: Vec<EdgeParametricInterval>,
) -> Vec<EdgeParametricInterval> {
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by(|left, right| {
        left.t_start
            .partial_cmp(&right.t_start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut merged: Vec<EdgeParametricInterval> = Vec::new();
    for interval in intervals {
        if interval.is_empty() {
            continue;
        }
        if let Some(last) = merged.last_mut() {
            if interval.t_start <= last.t_end + f32::EPSILON {
                last.t_end = last.t_end.max(interval.t_end);
                continue;
            }
        }
        merged.push(interval);
    }
    merged
}

/// Closed sub-intervals of `[0, 1]` after removing open intervals.
pub fn closed_segments_from_open_intervals(
    open_intervals: &[EdgeParametricInterval],
) -> Vec<EdgeParametricInterval> {
    if open_intervals.is_empty() {
        return vec![EdgeParametricInterval::new(0.0, 1.0)];
    }
    let merged = merge_edge_intervals(open_intervals.to_vec());
    let mut closed = Vec::new();
    let mut cursor = 0.0;
    for open in merged {
        if open.t_start > cursor + f32::EPSILON {
            closed.push(EdgeParametricInterval::new(cursor, open.t_start));
        }
        cursor = open.t_end.max(cursor);
    }
    if cursor < 1.0 - f32::EPSILON {
        closed.push(EdgeParametricInterval::new(cursor, 1.0));
    }
    closed
}

impl EdgeParametricInterval {
    pub fn contains_t(self, t: f32) -> bool {
        t >= self.t_start - f32::EPSILON && t <= self.t_end + f32::EPSILON
    }
}

pub fn point_parametric_on_edge(point: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let edge = b - a;
    let len_sq = edge.length_squared();
    if len_sq <= f32::EPSILON {
        return None;
    }
    Some(((point - a).dot(edge) / len_sq).clamp(0.0, 1.0))
}

/// Minimum distance from `point` to closed sub-segments of edge `a→b`.
pub fn min_distance_to_closed_edge_segments(
    point: Vec2,
    a: Vec2,
    b: Vec2,
    open_intervals: &[EdgeParametricInterval],
) -> f32 {
    let closed = closed_segments_from_open_intervals(open_intervals);
    closed
        .iter()
        .map(|segment| distance_point_to_closed_segment(point, a, b, *segment))
        .fold(f32::INFINITY, f32::min)
}

fn distance_point_to_closed_segment(
    point: Vec2,
    a: Vec2,
    b: Vec2,
    segment: EdgeParametricInterval,
) -> f32 {
    let edge = b - a;
    let start = a + edge * segment.t_start;
    let end = a + edge * segment.t_end;
    distance_point_to_segment(point, start, end)
}

fn distance_point_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let edge = end - start;
    let len_sq = edge.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(start);
    }
    let t = ((point - start).dot(edge) / len_sq).clamp(0.0, 1.0);
    point.distance(start + edge * t)
}

/// Collect usable center opening intervals for one region edge from enabled exterior entrances.
pub fn collect_usable_entrance_openings_on_edge(
    space_registry: &SpaceRegistry,
    building_id: BuildingId,
    region_space: SpaceId,
    edge_index: usize,
    edge_a: Vec2,
    edge_b: Vec2,
    agent_radius: f32,
) -> Vec<EdgeParametricInterval> {
    let mut intervals = Vec::new();
    for (_portal_id, portal) in space_registry.portals() {
        if !entrance_opening_applies(portal, building_id, region_space, edge_index) {
            continue;
        }
        let Some(threshold) = portal.entrance_threshold_global_xz else {
            continue;
        };
        if let Some(interval) = usable_center_opening_interval_on_edge(
            edge_a,
            edge_b,
            threshold,
            portal.from_radius_meters,
            agent_radius,
        ) {
            intervals.push(interval);
        }
    }
    merge_edge_intervals(intervals)
}

fn entrance_opening_applies(
    portal: &PortalRecord,
    building_id: BuildingId,
    region_space: SpaceId,
    edge_index: usize,
) -> bool {
    portal.owning_building_id == Some(building_id)
        && portal.enabled
        && portal.portal_type == PortalType::ExteriorEntrance
        && portal.to_space == region_space
        && portal.entrance_owning_edge_index == Some(edge_index as u32)
}

/// Minimum distance from an interior point to closed boundary geometry (opening-aware).
pub fn min_interior_closed_boundary_clearance_meters(
    point: Vec2,
    polygon: &[Vec2],
    space_registry: &SpaceRegistry,
    building_id: BuildingId,
    region_space: SpaceId,
    agent_radius: f32,
) -> f32 {
    if polygon.len() < 2 {
        return f32::INFINITY;
    }
    let mut min_dist = f32::INFINITY;
    for edge_index in 0..polygon.len() {
        let a = polygon[edge_index];
        let b = polygon[(edge_index + 1) % polygon.len()];
        let open_intervals = collect_usable_entrance_openings_on_edge(
            space_registry,
            building_id,
            region_space,
            edge_index,
            a,
            b,
            agent_radius,
        );
        let edge_dist = min_distance_to_closed_edge_segments(point, a, b, &open_intervals);
        min_dist = min_dist.min(edge_dist);
    }
    min_dist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usable_interval_none_when_agent_wider_than_opening() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 0.0);
        let threshold = Vec2::new(5.0, 0.0);
        assert!(usable_center_opening_interval_on_edge(a, b, threshold, 0.5, 0.68).is_none());
    }

    #[test]
    fn closed_segments_split_open_interval() {
        let open = vec![EdgeParametricInterval::new(0.4, 0.6)];
        let closed = closed_segments_from_open_intervals(&open);
        assert_eq!(closed.len(), 2);
        assert!((closed[0].t_end - 0.4).abs() < 1e-4);
        assert!((closed[1].t_start - 0.6).abs() < 1e-4);
    }

    #[test]
    fn merge_overlapping_intervals() {
        let merged = merge_edge_intervals(vec![
            EdgeParametricInterval::new(0.2, 0.5),
            EdgeParametricInterval::new(0.45, 0.7),
        ]);
        assert_eq!(merged.len(), 1);
        assert!((merged[0].t_start - 0.2).abs() < 1e-4);
        assert!((merged[0].t_end - 0.7).abs() < 1e-4);
    }
}
