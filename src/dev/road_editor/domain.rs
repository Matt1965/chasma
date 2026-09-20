use bevy::prelude::*;

use crate::world::{
    Road, RoadControlPoint, RoadId, RoadNetwork, RoadStyleId, endpoint_has_attachment,
    refresh_tee_branches_for_host, remove_road_and_cleanup_junctions, sample_road_polyline,
    try_snap_endpoint,
};

pub const ROAD_PICK_DISTANCE_M: f32 = 6.0;
pub const CONTROL_POINT_PICK_DISTANCE_M: f32 = 4.0;
pub const SPLINE_SAMPLE_SPACING_M: f32 = 2.0;

#[derive(Debug, Clone, PartialEq)]
pub struct SegmentHit {
    pub road_id: RoadId,
    pub segment_index: usize,
    pub position: Vec2,
}

pub fn generate_road_id(network: &RoadNetwork) -> RoadId {
    let mut index = network.roads.len().max(1);
    loop {
        let candidate = RoadId::new(format!("road_{index}"));
        if !network.roads.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

pub fn road_label(road: &Road) -> String {
    if road.display_name.trim().is_empty() {
        road.id.to_string()
    } else {
        road.display_name.clone()
    }
}

pub fn new_authored_road(
    id: RoadId,
    display_name: String,
    style: RoadStyleId,
    control_points: Vec<RoadControlPoint>,
) -> Road {
    Road {
        id,
        display_name,
        style,
        style_overrides: Default::default(),
        control_points,
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    }
}

pub fn pick_road_at(network: &RoadNetwork, xz: Vec2) -> Option<RoadId> {
    let mut best: Option<(RoadId, f32)> = None;
    for road in network.roads.values() {
        if road.control_points.len() < 2 {
            continue;
        }
        let distance = distance_to_road_polyline(road, xz);
        if distance <= ROAD_PICK_DISTANCE_M {
            if best.as_ref().map(|(_, d)| distance < *d).unwrap_or(true) {
                best = Some((road.id.clone(), distance));
            }
        }
    }
    best.map(|(id, _)| id)
}

pub fn pick_control_point_at(road: &Road, xz: Vec2) -> Option<usize> {
    road.control_points
        .iter()
        .enumerate()
        .filter_map(|(index, point)| {
            let distance = point.xz().distance(xz);
            if distance <= CONTROL_POINT_PICK_DISTANCE_M {
                Some((index, distance))
            } else {
                None
            }
        })
        .min_by(|(_, lhs), (_, rhs)| lhs.total_cmp(rhs))
        .map(|(index, _)| index)
}

pub fn pick_segment_for_insert(network: &RoadNetwork, xz: Vec2) -> Option<SegmentHit> {
    let mut best: Option<(SegmentHit, f32)> = None;
    for road in network.roads.values() {
        if road.control_points.len() < 2 {
            continue;
        }
        for segment_index in 0..road.control_points.len() - 1 {
            let start = road.control_points[segment_index].xz();
            let end = road.control_points[segment_index + 1].xz();
            let (projection, distance) = project_point_to_segment(xz, start, end);
            if distance <= ROAD_PICK_DISTANCE_M {
                let hit = SegmentHit {
                    road_id: road.id.clone(),
                    segment_index,
                    position: projection,
                };
                if best.as_ref().map(|(_, d)| distance < *d).unwrap_or(true) {
                    best = Some((hit, distance));
                }
            }
        }
    }
    best.map(|(hit, _)| hit)
}

pub fn insert_control_point(
    network: &mut RoadNetwork,
    road_id: &RoadId,
    segment_index: usize,
    position: Vec2,
) -> Result<usize, String> {
    let road = network
        .roads
        .get_mut(road_id)
        .ok_or_else(|| format!("road {road_id} not found"))?;
    if segment_index >= road.control_points.len().saturating_sub(1) {
        return Err("segment index out of range".into());
    }
    let insert_index = segment_index + 1;
    road.control_points.insert(insert_index, RoadControlPoint::new(position.x, position.y));
    refresh_tee_branches_for_host(network, road_id);
    Ok(insert_index)
}

pub fn move_control_point(road: &mut Road, index: usize, xz: Vec2) -> Result<(), String> {
    let point = road
        .control_points
        .get_mut(index)
        .ok_or_else(|| "control point index out of range".to_string())?;
    point.x = xz.x;
    point.z = xz.y;
    Ok(())
}

pub fn delete_control_point(road: &mut Road, index: usize) -> Result<(), String> {
    if road.control_points.len() <= 2 {
        return Err("roads must keep at least two control points".into());
    }
    if index >= road.control_points.len() {
        return Err("control point index out of range".into());
    }
    if endpoint_has_attachment(road, index) {
        return Err(
            "cannot delete an endpoint attached to a junction; detach the junction first".into(),
        );
    }
    road.control_points.remove(index);
    Ok(())
}

pub fn extend_road_start(road: &mut Road, point: RoadControlPoint) {
    road.control_points.insert(0, point);
}

pub fn extend_road_end(road: &mut Road, point: RoadControlPoint) {
    road.control_points.push(point);
}

pub fn delete_road(network: &mut RoadNetwork, road_id: &RoadId) -> bool {
    remove_road_and_cleanup_junctions(network, road_id)
}

pub fn finish_create_road(
    network: &mut RoadNetwork,
    display_name: String,
    style: RoadStyleId,
    points: Vec<RoadControlPoint>,
) -> Result<RoadId, String> {
    if points.len() < 2 {
        return Err("roads require at least two control points".into());
    }
    let start_xz = points.first().map(|point| point.xz());
    let end_xz = points.last().map(|point| point.xz());
    let id = generate_road_id(network);
    let road = new_authored_road(id.clone(), display_name, style, points);
    network.roads.insert(id.clone(), road);
    if let Some(xz) = start_xz {
        try_snap_endpoint(network, &id, true, xz)?;
    }
    if let Some(xz) = end_xz {
        try_snap_endpoint(network, &id, false, xz)?;
    }
    Ok(id)
}

pub fn cycle_road_style(style: RoadStyleId) -> RoadStyleId {
    match style {
        RoadStyleId::Trail => RoadStyleId::DirtRoad,
        RoadStyleId::DirtRoad => RoadStyleId::MajorRoad,
        RoadStyleId::MajorRoad => RoadStyleId::Trail,
    }
}

pub fn style_debug_color(style: RoadStyleId) -> Color {
    match style {
        RoadStyleId::Trail => Color::srgba(0.55, 0.75, 0.45, 0.95),
        RoadStyleId::DirtRoad => Color::srgba(0.72, 0.58, 0.36, 0.95),
        RoadStyleId::MajorRoad => Color::srgba(0.82, 0.82, 0.86, 0.95),
    }
}

fn distance_to_road_polyline(road: &Road, xz: Vec2) -> f32 {
    sample_road_polyline(road, SPLINE_SAMPLE_SPACING_M)
        .iter()
        .map(|sample| sample.position.distance(xz))
        .min_by(|lhs, rhs| lhs.total_cmp(rhs))
        .unwrap_or(f32::MAX)
}

fn project_point_to_segment(point: Vec2, start: Vec2, end: Vec2) -> (Vec2, f32) {
    let segment = end - start;
    let length_sq = segment.length_squared();
    if length_sq <= f32::EPSILON {
        return (start, point.distance(start));
    }
    let t = ((point - start).dot(segment) / length_sq).clamp(0.0, 1.0);
    let projection = start + segment * t;
    (projection, point.distance(projection))
}
