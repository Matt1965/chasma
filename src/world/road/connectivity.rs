use bevy::prelude::*;

use super::attachment::{RoadEndpointAttachment, RoadTeeAttachment};
use super::constants::{
    JUNCTION_SNAP_RADIUS_M, ROAD_CONNECTIVITY_SAMPLE_SPACING_M,
};
use super::id::{JunctionId, RoadId};
use super::junction::{Junction, JunctionMember, JunctionMemberRole};
use super::network::RoadNetwork;
use super::road::Road;
use super::spline::{project_point_onto_road_spline, sample_road_spline_at_t};

/// Kind of junction snap candidate shown during authoring.
#[derive(Debug, Clone, PartialEq)]
pub enum SnapCandidateKind {
    Endpoint {
        road_id: RoadId,
        is_start: bool,
        junction_id: Option<JunctionId>,
    },
    Tee {
        host_road_id: RoadId,
        host_t: f32,
    },
}

/// Transient snap preview for road endpoint placement.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapCandidate {
    pub kind: SnapCandidateKind,
    pub position: Vec2,
    pub distance_m: f32,
}

pub fn generate_junction_id(network: &RoadNetwork) -> JunctionId {
    let mut index = network.junctions.len().max(1);
    loop {
        let candidate = JunctionId::new(format!("junction_{index}"));
        if !network.junctions.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

pub fn endpoint_index(point_count: usize, is_start: bool) -> usize {
    if is_start {
        0
    } else {
        point_count - 1
    }
}

pub fn is_road_endpoint_index(index: usize, point_count: usize) -> bool {
    point_count >= 2 && (index == 0 || index == point_count - 1)
}

pub fn endpoint_world_position(road: &Road, is_start: bool) -> Option<Vec2> {
    let index = if is_start {
        0
    } else {
        road.control_points.len() - 1
    };
    road.control_points.get(index).map(|point| point.xz())
}

pub fn set_endpoint_world_position(road: &mut Road, is_start: bool, xz: Vec2) {
    let index = endpoint_index(road.control_points.len(), is_start);
    if let Some(point) = road.control_points.get_mut(index) {
        point.x = xz.x;
        point.z = xz.y;
    }
}

pub fn junction_id_for_endpoint(road: &Road, is_start: bool) -> Option<JunctionId> {
    let attachment = if is_start {
        road.start_attachment.as_ref()
    } else {
        road.end_attachment.as_ref()
    };
    attachment.map(|value| value.junction_id.clone())
}

pub fn find_snap_candidate(
    network: &RoadNetwork,
    active_road_id: &RoadId,
    active_is_start: bool,
    xz: Vec2,
) -> Option<SnapCandidate> {
    let mut endpoint_best: Option<SnapCandidate> = None;
    for road in network.roads.values() {
        if road.id == *active_road_id {
            continue;
        }
        for (is_start, attachment) in [
            (true, road.start_attachment.as_ref()),
            (false, road.end_attachment.as_ref()),
        ] {
            let Some(endpoint) = endpoint_world_position(road, is_start) else {
                continue;
            };
            let distance = endpoint.distance(xz);
            if distance > JUNCTION_SNAP_RADIUS_M {
                continue;
            }
            let candidate = SnapCandidate {
                kind: SnapCandidateKind::Endpoint {
                    road_id: road.id.clone(),
                    is_start,
                    junction_id: attachment.map(|value| value.junction_id.clone()),
                },
                position: endpoint,
                distance_m: distance,
            };
            if endpoint_best
                .as_ref()
                .map(|current| distance < current.distance_m)
                .unwrap_or(true)
            {
                endpoint_best = Some(candidate);
            }
        }
    }

    if endpoint_best.is_some() {
        return endpoint_best;
    }

    let mut tee_best: Option<SnapCandidate> = None;
    for road in network.roads.values() {
        if road.id == *active_road_id {
            continue;
        }
        if road.control_points.len() < 2 {
            continue;
        }
        let projection = project_point_onto_road_spline(
            road,
            xz,
            ROAD_CONNECTIVITY_SAMPLE_SPACING_M,
        )?;
        if projection.distance_to_point > JUNCTION_SNAP_RADIUS_M {
            continue;
        }
        if is_near_road_endpoint(road, projection.position) {
            continue;
        }
        let candidate = SnapCandidate {
            kind: SnapCandidateKind::Tee {
                host_road_id: road.id.clone(),
                host_t: projection.normalized_t,
            },
            position: projection.position,
            distance_m: projection.distance_to_point,
        };
        if tee_best
            .as_ref()
            .map(|current| projection.distance_to_point < current.distance_m)
            .unwrap_or(true)
        {
            tee_best = Some(candidate);
        }
    }
    tee_best
}

pub fn apply_snap_candidate(
    network: &mut RoadNetwork,
    active_road_id: &RoadId,
    active_is_start: bool,
    candidate: &SnapCandidate,
) -> Result<JunctionId, String> {
    match &candidate.kind {
        SnapCandidateKind::Endpoint {
            road_id,
            is_start,
            junction_id,
        } => join_endpoint_to_endpoint(
            network,
            active_road_id,
            active_is_start,
            road_id,
            *is_start,
            junction_id.clone(),
            candidate.position,
        ),
        SnapCandidateKind::Tee {
            host_road_id,
            host_t,
        } => create_tee_junction(
            network,
            active_road_id,
            active_is_start,
            host_road_id,
            *host_t,
            candidate.position,
        ),
    }
}

pub fn try_snap_endpoint(
    network: &mut RoadNetwork,
    road_id: &RoadId,
    is_start: bool,
    xz: Vec2,
) -> Result<Option<JunctionId>, String> {
    let candidate = find_snap_candidate(network, road_id, is_start, xz);
    if let Some(candidate) = candidate {
        let junction_id = apply_snap_candidate(network, road_id, is_start, &candidate)?;
        Ok(Some(junction_id))
    } else {
        set_endpoint_world_position(
            network
                .roads
                .get_mut(road_id)
                .ok_or_else(|| format!("road {road_id} not found"))?,
            is_start,
            xz,
        );
        Ok(None)
    }
}

pub fn join_endpoint_to_endpoint(
    network: &mut RoadNetwork,
    active_road_id: &RoadId,
    active_is_start: bool,
    target_road_id: &RoadId,
    target_is_start: bool,
    existing_junction_id: Option<JunctionId>,
    position: Vec2,
) -> Result<JunctionId, String> {
    if active_road_id == target_road_id && active_is_start == target_is_start {
        return Err("cannot snap a road endpoint to itself".into());
    }

    let active_junction = junction_id_for_endpoint(
        network
            .roads
            .get(active_road_id)
            .ok_or_else(|| format!("road {active_road_id} not found"))?,
        active_is_start,
    );
    if active_junction.as_ref() != existing_junction_id.as_ref() {
        remove_endpoint_from_junction(network, active_road_id, active_is_start)?;
    } else if existing_junction_id.is_some() {
        move_endpoint_junction_group(network, existing_junction_id.as_ref().expect("junction"), position)?;
        return Ok(existing_junction_id.expect("junction"));
    }

    let junction_id = if let Some(junction_id) = existing_junction_id {
        junction_id
    } else {
        let junction_id = generate_junction_id(network);
        network.junctions.insert(
            junction_id.clone(),
            Junction {
                id: junction_id.clone(),
                members: vec![JunctionMember {
                    road_id: target_road_id.clone(),
                    role: endpoint_role(target_is_start),
                    host_t: None,
                }],
            },
        );
        set_road_endpoint_attachment(network, target_road_id, target_is_start, junction_id.clone())?;
        junction_id
    };

    add_endpoint_member_if_missing(network, &junction_id, active_road_id, active_is_start)?;
    set_road_endpoint_attachment(network, active_road_id, active_is_start, junction_id.clone())?;
    move_endpoint_junction_group(network, &junction_id, position)?;
    Ok(junction_id)
}

pub fn create_tee_junction(
    network: &mut RoadNetwork,
    branch_road_id: &RoadId,
    branch_is_start: bool,
    host_road_id: &RoadId,
    host_t: f32,
    position: Vec2,
) -> Result<JunctionId, String> {
    if branch_road_id == host_road_id {
        return Err("branch road cannot tee into itself".into());
    }
    if !network.roads.contains_key(host_road_id) {
        return Err(format!("host road {host_road_id} not found"));
    }

    remove_endpoint_from_junction(network, branch_road_id, branch_is_start)?;

    let junction_id = generate_junction_id(network);
    network.junctions.insert(
        junction_id.clone(),
        Junction {
            id: junction_id.clone(),
            members: vec![
                JunctionMember {
                    road_id: host_road_id.clone(),
                    role: JunctionMemberRole::TeeHost,
                    host_t: Some(host_t),
                },
                JunctionMember {
                    road_id: branch_road_id.clone(),
                    role: JunctionMemberRole::TeeBranch,
                    host_t: None,
                },
            ],
        },
    );

    let host_road = network
        .roads
        .get_mut(host_road_id)
        .ok_or_else(|| format!("host road {host_road_id} not found"))?;
    host_road.tee_attachments.push(RoadTeeAttachment {
        junction_id: junction_id.clone(),
        host_road_id: host_road_id.clone(),
        host_t,
    });

    set_road_endpoint_attachment(network, branch_road_id, branch_is_start, junction_id.clone())?;
    set_endpoint_world_position(
        network
            .roads
            .get_mut(branch_road_id)
            .ok_or_else(|| format!("branch road {branch_road_id} not found"))?,
        branch_is_start,
        position,
    );
    Ok(junction_id)
}

pub fn detach_junction(network: &mut RoadNetwork, junction_id: &JunctionId) -> Result<(), String> {
    let junction = network
        .junctions
        .get(junction_id)
        .ok_or_else(|| format!("junction {junction_id} not found"))?
        .clone();

    for member in &junction.members {
        let road = network
            .roads
            .get_mut(&member.road_id)
            .ok_or_else(|| format!("road {} not found", member.road_id))?;
        match member.role {
            JunctionMemberRole::EndpointStart => road.start_attachment = None,
            JunctionMemberRole::EndpointEnd => road.end_attachment = None,
            JunctionMemberRole::TeeHost => {
                road.tee_attachments
                    .retain(|tee| tee.junction_id != *junction_id);
            }
            JunctionMemberRole::TeeBranch => {
                if road
                    .start_attachment
                    .as_ref()
                    .is_some_and(|attachment| attachment.junction_id == *junction_id)
                {
                    road.start_attachment = None;
                }
                if road
                    .end_attachment
                    .as_ref()
                    .is_some_and(|attachment| attachment.junction_id == *junction_id)
                {
                    road.end_attachment = None;
                }
            }
        }
    }

    network.junctions.remove(junction_id);
    Ok(())
}

pub fn remove_road_and_cleanup_junctions(network: &mut RoadNetwork, road_id: &RoadId) -> bool {
    if !network.roads.contains_key(road_id) {
        return false;
    }

    let junction_ids = collect_junction_ids_for_road(network, road_id);
    network.roads.remove(road_id);
    network
        .crossings
        .retain(|crossing| crossing.road_a != *road_id && crossing.road_b != *road_id);

    for junction_id in junction_ids {
        if let Some(junction) = network.junctions.get_mut(&junction_id) {
            junction.members.retain(|member| member.road_id != *road_id);
        }
        for road in network.roads.values_mut() {
            road.tee_attachments
                .retain(|tee| tee.host_road_id != *road_id || tee.junction_id != junction_id);
        }

        let should_remove = network
            .junctions
            .get(&junction_id)
            .map(|junction| junction.members.len() < 2)
            .unwrap_or(true);
        if should_remove {
            detach_junction(network, &junction_id).ok();
        }
    }

    true
}

pub fn move_endpoint_junction_group(
    network: &mut RoadNetwork,
    junction_id: &JunctionId,
    xz: Vec2,
) -> Result<(), String> {
    let junction = network
        .junctions
        .get(junction_id)
        .ok_or_else(|| format!("junction {junction_id} not found"))?
        .clone();
    for member in &junction.members {
        match member.role {
            JunctionMemberRole::EndpointStart => {
                set_endpoint_world_position(
                    network
                        .roads
                        .get_mut(&member.road_id)
                        .ok_or_else(|| format!("road {} not found", member.road_id))?,
                    true,
                    xz,
                );
            }
            JunctionMemberRole::EndpointEnd => {
                set_endpoint_world_position(
                    network
                        .roads
                        .get_mut(&member.road_id)
                        .ok_or_else(|| format!("road {} not found", member.road_id))?,
                    false,
                    xz,
                );
            }
            JunctionMemberRole::TeeBranch => {
                if let Some(road) = network.roads.get_mut(&member.road_id) {
                    if road.start_attachment.as_ref().is_some_and(|attachment| {
                        attachment.junction_id == *junction_id
                    }) {
                        set_endpoint_world_position(road, true, xz);
                    } else if road.end_attachment.as_ref().is_some_and(|attachment| {
                        attachment.junction_id == *junction_id
                    }) {
                        set_endpoint_world_position(road, false, xz);
                    }
                }
            }
            JunctionMemberRole::TeeHost => {}
        }
    }
    Ok(())
}

pub fn move_connected_endpoint(
    network: &mut RoadNetwork,
    road_id: &RoadId,
    index: usize,
    xz: Vec2,
) -> Result<(), String> {
    let road = network
        .roads
        .get(road_id)
        .ok_or_else(|| format!("road {road_id} not found"))?;
    if !is_road_endpoint_index(index, road.control_points.len()) {
        let road = network.roads.get_mut(road_id).expect("road exists");
        let point = road
            .control_points
            .get_mut(index)
            .ok_or_else(|| "control point index out of range".to_string())?;
        point.x = xz.x;
        point.z = xz.y;
        refresh_tee_branches_for_host(network, road_id);
        return Ok(());
    }

    let is_start = index == 0;
    if let Some(junction_id) = junction_id_for_endpoint(road, is_start) {
        move_endpoint_junction_group(network, &junction_id, xz)?;
    } else {
        set_endpoint_world_position(
            network.roads.get_mut(road_id).expect("road exists"),
            is_start,
            xz,
        );
    }
    refresh_tee_branches_for_host(network, road_id);
    Ok(())
}

pub fn finalize_endpoint_drag(
    network: &mut RoadNetwork,
    road_id: &RoadId,
    index: usize,
    xz: Vec2,
) -> Result<Option<JunctionId>, String> {
    let road = network
        .roads
        .get(road_id)
        .ok_or_else(|| format!("road {road_id} not found"))?;
    if !is_road_endpoint_index(index, road.control_points.len()) {
        move_connected_endpoint(network, road_id, index, xz)?;
        return Ok(None);
    }
    let is_start = index == 0;
    if junction_id_for_endpoint(road, is_start).is_some() {
        move_endpoint_junction_group(
            network,
            &junction_id_for_endpoint(road, is_start).expect("junction"),
            xz,
        )?;
        if let Some(candidate) = find_snap_candidate(network, road_id, is_start, xz) {
            return apply_snap_candidate(network, road_id, is_start, &candidate).map(Some);
        }
        return Ok(junction_id_for_endpoint(
            network.roads.get(road_id).expect("road"),
            is_start,
        ));
    }
    try_snap_endpoint(network, road_id, is_start, xz)
}

pub fn refresh_tee_branches_for_host(network: &mut RoadNetwork, host_road_id: &RoadId) {
    let host_road = network.roads.get(host_road_id);
    if host_road.is_none() {
        return;
    }
    let tees = host_road
        .expect("host road")
        .tee_attachments
        .iter()
        .map(|tee| (tee.junction_id.clone(), tee.host_t))
        .collect::<Vec<_>>();

    for (junction_id, host_t) in tees {
        let position = network
            .roads
            .get(host_road_id)
            .and_then(|road| sample_road_spline_at_t(road, host_t))
            .map(|sample| sample.position);
        if let Some(position) = position {
            move_endpoint_junction_group(network, &junction_id, position).ok();
        }
    }
}

pub fn refresh_all_tee_branches(network: &mut RoadNetwork) {
    let host_ids = network
        .roads
        .values()
        .filter(|road| !road.tee_attachments.is_empty())
        .map(|road| road.id.clone())
        .collect::<Vec<_>>();
    for host_id in host_ids {
        refresh_tee_branches_for_host(network, &host_id);
    }
}

pub fn junction_world_position(network: &RoadNetwork, junction_id: &JunctionId) -> Option<Vec2> {
    let junction = network.junctions.get(junction_id)?;
    let mut positions = Vec::new();
    for member in &junction.members {
        match member.role {
            JunctionMemberRole::EndpointStart => {
                if let Some(road) = network.roads.get(&member.road_id) {
                    if let Some(position) = endpoint_world_position(road, true) {
                        positions.push(position);
                    }
                }
            }
            JunctionMemberRole::EndpointEnd => {
                if let Some(road) = network.roads.get(&member.road_id) {
                    if let Some(position) = endpoint_world_position(road, false) {
                        positions.push(position);
                    }
                }
            }
            JunctionMemberRole::TeeHost => {
                if let Some(host_t) = member.host_t {
                    if let Some(road) = network.roads.get(&member.road_id) {
                        if let Some(sample) = sample_road_spline_at_t(road, host_t) {
                            positions.push(sample.position);
                        }
                    }
                }
            }
            JunctionMemberRole::TeeBranch => {
                if let Some(road) = network.roads.get(&member.road_id) {
                    if road
                        .start_attachment
                        .as_ref()
                        .is_some_and(|attachment| attachment.junction_id == *junction_id)
                    {
                        if let Some(position) = endpoint_world_position(road, true) {
                            positions.push(position);
                        }
                    } else if road
                        .end_attachment
                        .as_ref()
                        .is_some_and(|attachment| attachment.junction_id == *junction_id)
                    {
                        if let Some(position) = endpoint_world_position(road, false) {
                            positions.push(position);
                        }
                    }
                }
            }
        }
    }
    if positions.is_empty() {
        return None;
    }
    let sum = positions.iter().fold(Vec2::ZERO, |acc, position| acc + *position);
    Some(sum / positions.len() as f32)
}

pub fn persisted_junction_positions(network: &RoadNetwork) -> Vec<(JunctionId, Vec2)> {
    network
        .junctions
        .keys()
        .filter_map(|junction_id| {
            junction_world_position(network, junction_id)
                .map(|position| (junction_id.clone(), position))
        })
        .collect()
}

pub fn endpoint_has_attachment(road: &Road, index: usize) -> bool {
    if index == 0 {
        road.start_attachment.is_some()
    } else if index + 1 == road.control_points.len() {
        road.end_attachment.is_some()
    } else {
        false
    }
}

fn endpoint_role(is_start: bool) -> JunctionMemberRole {
    if is_start {
        JunctionMemberRole::EndpointStart
    } else {
        JunctionMemberRole::EndpointEnd
    }
}

fn set_road_endpoint_attachment(
    network: &mut RoadNetwork,
    road_id: &RoadId,
    is_start: bool,
    junction_id: JunctionId,
) -> Result<(), String> {
    let road = network
        .roads
        .get_mut(road_id)
        .ok_or_else(|| format!("road {road_id} not found"))?;
    let attachment = RoadEndpointAttachment {
        junction_id,
        is_start,
    };
    if is_start {
        road.start_attachment = Some(attachment);
    } else {
        road.end_attachment = Some(attachment);
    }
    Ok(())
}

fn add_endpoint_member_if_missing(
    network: &mut RoadNetwork,
    junction_id: &JunctionId,
    road_id: &RoadId,
    is_start: bool,
) -> Result<(), String> {
    let junction = network
        .junctions
        .get_mut(junction_id)
        .ok_or_else(|| format!("junction {junction_id} not found"))?;
    if junction
        .members
        .iter()
        .any(|member| member.road_id == *road_id)
    {
        return Ok(());
    }
    junction.members.push(JunctionMember {
        road_id: road_id.clone(),
        role: endpoint_role(is_start),
        host_t: None,
    });
    Ok(())
}

fn collect_junction_ids_for_road(network: &RoadNetwork, road_id: &RoadId) -> Vec<JunctionId> {
    let mut junction_ids = Vec::new();
    if let Some(road) = network.roads.get(road_id) {
        if let Some(attachment) = &road.start_attachment {
            junction_ids.push(attachment.junction_id.clone());
        }
        if let Some(attachment) = &road.end_attachment {
            junction_ids.push(attachment.junction_id.clone());
        }
        for tee in &road.tee_attachments {
            junction_ids.push(tee.junction_id.clone());
        }
    }
    for junction in network.junctions.values() {
        if junction.members.iter().any(|member| member.road_id == *road_id) {
            junction_ids.push(junction.id.clone());
        }
    }
    junction_ids.sort();
    junction_ids.dedup();
    junction_ids
}

fn remove_endpoint_from_junction(
    network: &mut RoadNetwork,
    road_id: &RoadId,
    is_start: bool,
) -> Result<(), String> {
    let road = network
        .roads
        .get(road_id)
        .ok_or_else(|| format!("road {road_id} not found"))?;
    let Some(junction_id) = junction_id_for_endpoint(road, is_start) else {
        return Ok(());
    };

    if let Some(junction) = network.junctions.get_mut(&junction_id) {
        junction.members.retain(|member| member.road_id != *road_id);
    }
    let road = network.roads.get_mut(road_id).expect("road exists");
    if is_start {
        road.start_attachment = None;
    } else {
        road.end_attachment = None;
    }

    let should_remove = network
        .junctions
        .get(&junction_id)
        .map(|junction| junction.members.len() < 2)
        .unwrap_or(true);
    if should_remove {
        detach_junction(network, &junction_id)?;
    }
    Ok(())
}

fn is_near_road_endpoint(road: &Road, position: Vec2) -> bool {
    for is_start in [true, false] {
        if let Some(endpoint) = endpoint_world_position(road, is_start) {
            if endpoint.distance(position) <= JUNCTION_SNAP_RADIUS_M {
                return true;
            }
        }
    }
    false
}
