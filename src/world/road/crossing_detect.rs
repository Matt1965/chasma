use bevy::prelude::*;

use super::constants::{
    CROSSING_DEDUP_RADIUS_M, CROSSING_ENDPOINT_EXCLUSION_M, CROSSING_PARAMETER_TOLERANCE,
    ROAD_CONNECTIVITY_SAMPLE_SPACING_M,
};
use super::connectivity::{endpoint_world_position, persisted_junction_positions};
use super::crossing::{RoadCrossingKind, RoadCrossingOverride};
use super::id::RoadId;
use super::network::RoadNetwork;
use super::road::Road;
use super::spline::{project_point_onto_road_spline, sample_road_polyline};

/// One derived ground-level crossing between two distinct roads.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedCrossing {
    pub road_a: RoadId,
    pub road_b: RoadId,
    pub position: Vec2,
    pub road_a_t: f32,
    pub road_b_t: f32,
    pub connected: bool,
}

/// Deterministically derive incidental ground-level crossings from road spline geometry.
///
/// Persisted endpoint and tee junctions are excluded. Crossing overrides may suppress
/// connectivity while still reporting the geometric crossing.
pub fn derive_ground_crossings(network: &RoadNetwork) -> Vec<DerivedCrossing> {
    let road_ids = network
        .roads
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let junction_positions = persisted_junction_positions(network);
    let mut raw_hits = Vec::new();

    for left_index in 0..road_ids.len() {
        for right_index in left_index + 1..road_ids.len() {
            let road_a_id = &road_ids[left_index];
            let road_b_id = &road_ids[right_index];
            let road_a = network.roads.get(road_a_id).expect("road");
            let road_b = network.roads.get(road_b_id).expect("road");
            collect_crossings_between_roads(
                road_a,
                road_b,
                &junction_positions,
                &mut raw_hits,
            );
        }
    }

    dedupe_crossings(raw_hits, network)
}

fn collect_crossings_between_roads(
    road_a: &Road,
    road_b: &Road,
    junction_positions: &[(super::id::JunctionId, Vec2)],
    out: &mut Vec<DerivedCrossing>,
) {
    if road_a.control_points.len() < 2 || road_b.control_points.len() < 2 {
        return;
    }

    let samples_a = sample_road_polyline(road_a, ROAD_CONNECTIVITY_SAMPLE_SPACING_M);
    let samples_b = sample_road_polyline(road_b, ROAD_CONNECTIVITY_SAMPLE_SPACING_M);

    for window_a in samples_a.windows(2) {
        for window_b in samples_b.windows(2) {
            if let Some(position) = segment_intersection(
                window_a[0].position,
                window_a[1].position,
                window_b[0].position,
                window_b[1].position,
            ) {
                if is_near_any_endpoint(road_a, position)
                    || is_near_any_endpoint(road_b, position)
                    || is_near_persisted_junction(junction_positions, position)
                {
                    continue;
                }
                let projection_a = project_point_onto_road_spline(
                    road_a,
                    position,
                    ROAD_CONNECTIVITY_SAMPLE_SPACING_M,
                );
                let projection_b = project_point_onto_road_spline(
                    road_b,
                    position,
                    ROAD_CONNECTIVITY_SAMPLE_SPACING_M,
                );
                if let (Some(projection_a), Some(projection_b)) = (projection_a, projection_b) {
                    out.push(DerivedCrossing {
                        road_a: road_a.id.clone(),
                        road_b: road_b.id.clone(),
                        position,
                        road_a_t: projection_a.normalized_t,
                        road_b_t: projection_b.normalized_t,
                        connected: true,
                    });
                }
            }
        }
    }
}

fn dedupe_crossings(
    hits: Vec<DerivedCrossing>,
    network: &RoadNetwork,
) -> Vec<DerivedCrossing> {
    let mut deduped = Vec::new();
    for hit in hits {
        let canonical = canonical_pair(&hit.road_a, &hit.road_b);
        if deduped.iter().any(|current: &DerivedCrossing| {
            canonical_pair(&current.road_a, &current.road_b) == canonical
                && current.position.distance(hit.position) <= CROSSING_DEDUP_RADIUS_M
        }) {
            continue;
        }
        deduped.push(hit);
    }

    for crossing in &mut deduped {
        crossing.connected = crossing_connection_enabled(network, crossing);
    }
    deduped
}

fn crossing_connection_enabled(network: &RoadNetwork, crossing: &DerivedCrossing) -> bool {
    let canonical = canonical_pair(&crossing.road_a, &crossing.road_b);
    for override_entry in &network.crossings {
        let override_pair = canonical_pair(&override_entry.road_a, &override_entry.road_b);
        if override_pair != canonical {
            continue;
        }
        let matches_a = (override_entry.road_a_t - crossing.road_a_t).abs()
            <= CROSSING_PARAMETER_TOLERANCE;
        let matches_b = (override_entry.road_b_t - crossing.road_b_t).abs()
            <= CROSSING_PARAMETER_TOLERANCE;
        if matches_a && matches_b {
            return override_entry.kind == RoadCrossingKind::Connected;
        }
    }
    true
}

fn canonical_pair(road_a: &RoadId, road_b: &RoadId) -> (RoadId, RoadId) {
    if road_a.as_str() <= road_b.as_str() {
        (road_a.clone(), road_b.clone())
    } else {
        (road_b.clone(), road_a.clone())
    }
}

fn is_near_any_endpoint(road: &Road, position: Vec2) -> bool {
    for is_start in [true, false] {
        if let Some(endpoint) = endpoint_world_position(road, is_start) {
            if endpoint.distance(position) <= CROSSING_ENDPOINT_EXCLUSION_M {
                return true;
            }
        }
    }
    false
}

fn is_near_persisted_junction(junction_positions: &[(super::id::JunctionId, Vec2)], position: Vec2) -> bool {
    junction_positions
        .iter()
        .any(|(_, junction_position)| junction_position.distance(position) <= CROSSING_DEDUP_RADIUS_M)
}

fn segment_intersection(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
    let r = a2 - a1;
    let s = b2 - b1;
    let denominator = cross(r, s);
    if denominator.abs() <= 1e-6 {
        return None;
    }
    let qp = b1 - a1;
    let t = cross(qp, s) / denominator;
    let u = cross(qp, r) / denominator;
    if t < 0.0 || t > 1.0 || u < 0.0 || u > 1.0 {
        return None;
    }
    Some(a1 + r * t)
}

fn cross(lhs: Vec2, rhs: Vec2) -> f32 {
    lhs.x * rhs.y - lhs.y * rhs.x
}
