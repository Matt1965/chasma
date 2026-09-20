use std::collections::{BTreeMap, BTreeSet};

use super::attachment::{RoadEndpointAttachment, RoadTeeAttachment};
use super::crossing::RoadCrossingOverride;
use super::error::RoadError;
use super::id::{JunctionId, RoadId};
use super::junction::{Junction, JunctionMemberRole};
use super::network::{RoadNetwork, ROAD_NETWORK_SCHEMA_VERSION};
use super::road::Road;
use super::style::{RoadStyleDefaults, RoadStyleId};

pub fn validate_road_network(network: &RoadNetwork) -> Result<(), RoadError> {
    if network.version != ROAD_NETWORK_SCHEMA_VERSION {
        return Err(RoadError::UnsupportedSchemaVersion {
            found: network.version,
            expected: ROAD_NETWORK_SCHEMA_VERSION,
        });
    }

    for defaults in network.styles.values() {
        defaults.validate()?;
    }

    let mut seen_road_ids = BTreeSet::new();
    for road in network.roads.values() {
        if !seen_road_ids.insert(road.id.clone()) {
            return Err(RoadError::DuplicateRoadId(road.id.clone()));
        }
    }
    for (road_key, road) in &network.roads {
        if road_key != &road.id {
            return Err(RoadError::InvalidRoadId(format!(
                "road map key {road_key} does not match road id {road_id}",
                road_id = road.id
            )));
        }
        road.validate()?;
        if network.styles.get(&road.style).is_none() {
            return Err(RoadError::UnknownStyle(road.style));
        }
    }

    let mut seen_junction_ids = BTreeSet::new();
    for (junction_key, junction) in &network.junctions {
        if junction_key != &junction.id {
            return Err(RoadError::InvalidJunctionId(format!(
                "junction map key {junction_key} does not match junction id {junction_id}",
                junction_id = junction.id
            )));
        }
        if !seen_junction_ids.insert(junction.id.clone()) {
            return Err(RoadError::DuplicateJunctionId(junction.id.clone()));
        }
        junction.validate()?;
        validate_junction_members_unique(&junction.id, &junction.members)?;
        validate_junction_cardinality(&junction.id, &junction.members)?;
    }

    for junction in network.junctions.values() {
        for member in &junction.members {
            let road = network.roads.get(&member.road_id).ok_or_else(|| {
                RoadError::InvalidJunctionReference(format!(
                    "junction {} references missing road {}",
                    junction.id,
                    member.road_id
                ))
            })?;
            validate_member_matches_road(&junction.id, member, road)?;
        }
    }

    for road in network.roads.values() {
        validate_road_attachments(network, road)?;
    }

    for crossing in &network.crossings {
        validate_crossing(network, crossing)?;
    }

    Ok(())
}

fn validate_junction_cardinality(
    junction_id: &JunctionId,
    members: &[super::junction::JunctionMember],
) -> Result<(), RoadError> {
    if members.len() < 2 {
        return Err(RoadError::InvalidJunctionReference(format!(
            "junction {} must have at least two members",
            junction_id
        )));
    }
    let tee_hosts = members
        .iter()
        .filter(|member| member.role == JunctionMemberRole::TeeHost)
        .count();
    let tee_branches = members
        .iter()
        .filter(|member| member.role == JunctionMemberRole::TeeBranch)
        .count();
    if tee_hosts > 0 && tee_hosts != 1 {
        return Err(RoadError::InvalidJunctionReference(format!(
            "junction {} must have exactly one TeeHost member",
            junction_id
        )));
    }
    if tee_branches > 0 && tee_branches != 1 {
        return Err(RoadError::InvalidJunctionReference(format!(
            "junction {} must have exactly one TeeBranch member",
            junction_id
        )));
    }
    if tee_hosts == 1 && tee_branches != 1 {
        return Err(RoadError::InvalidJunctionReference(format!(
            "junction {} tee junction must include one TeeBranch member",
            junction_id
        )));
    }
    Ok(())
}

fn validate_junction_members_unique(
    junction_id: &JunctionId,
    members: &[super::junction::JunctionMember],
) -> Result<(), RoadError> {
    let mut seen = BTreeSet::new();
    for member in members {
        if !seen.insert(member.road_id.clone()) {
            return Err(RoadError::DuplicateJunctionMember {
                junction_id: junction_id.clone(),
                road_id: member.road_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_member_matches_road(
    junction_id: &JunctionId,
    member: &super::junction::JunctionMember,
    road: &Road,
) -> Result<(), RoadError> {
    match member.role {
        JunctionMemberRole::EndpointStart => {
            let attachment = road.start_attachment.as_ref().ok_or_else(|| {
                RoadError::AttachmentMismatch(format!(
                    "junction {} expects start attachment on road {}",
                    junction_id,
                    road.id
                ))
            })?;
            ensure_endpoint_attachment(junction_id, road, attachment, true)?;
        }
        JunctionMemberRole::EndpointEnd => {
            let attachment = road.end_attachment.as_ref().ok_or_else(|| {
                RoadError::AttachmentMismatch(format!(
                    "junction {} expects end attachment on road {}",
                    junction_id,
                    road.id
                ))
            })?;
            ensure_endpoint_attachment(junction_id, road, attachment, false)?;
        }
        JunctionMemberRole::TeeHost => {
            let host_t = member.host_t.ok_or_else(|| {
                RoadError::InvalidJunctionReference(format!(
                    "junction {} TeeHost member for road {} missing host_t",
                    junction_id,
                    road.id
                ))
            })?;
            let tee = road
                .tee_attachments
                .iter()
                .find(|tee| tee.junction_id == *junction_id)
                .ok_or_else(|| {
                    RoadError::AttachmentMismatch(format!(
                        "junction {} expects tee attachment on host road {}",
                        junction_id,
                        road.id
                    ))
                })?;
            if tee.host_road_id != road.id {
                return Err(RoadError::AttachmentMismatch(format!(
                    "junction {} tee host road mismatch for road {}",
                    junction_id,
                    road.id
                )));
            }
            if (tee.host_t - host_t).abs() > 1e-5 {
                return Err(RoadError::AttachmentMismatch(format!(
                    "junction {} tee host_t mismatch for road {}",
                    junction_id,
                    road.id
                )));
            }
        }
        JunctionMemberRole::TeeBranch => {
            let has_branch_attachment = road.start_attachment.as_ref().is_some_and(|a| a.junction_id == *junction_id)
                || road.end_attachment.as_ref().is_some_and(|a| a.junction_id == *junction_id)
                || road
                    .tee_attachments
                    .iter()
                    .any(|tee| tee.junction_id == *junction_id);
            if !has_branch_attachment {
                return Err(RoadError::AttachmentMismatch(format!(
                    "junction {} expects branch attachment on road {}",
                    junction_id,
                    road.id
                )));
            }
        }
    }
    Ok(())
}

fn ensure_endpoint_attachment(
    junction_id: &JunctionId,
    road: &Road,
    attachment: &RoadEndpointAttachment,
    is_start: bool,
) -> Result<(), RoadError> {
    if attachment.junction_id != *junction_id {
        return Err(RoadError::AttachmentMismatch(format!(
            "road {} endpoint attachment references junction {} instead of {}",
            road.id,
            attachment.junction_id,
            junction_id
        )));
    }
    if attachment.is_start != is_start {
        return Err(RoadError::AttachmentMismatch(format!(
            "road {} endpoint attachment start/end flag mismatch for junction {}",
            road.id,
            junction_id
        )));
    }
    Ok(())
}

fn validate_road_attachments(network: &RoadNetwork, road: &Road) -> Result<(), RoadError> {
    if let Some(attachment) = &road.start_attachment {
        ensure_junction_contains_endpoint_member(network, &attachment.junction_id, &road.id, true)?;
    }
    if let Some(attachment) = &road.end_attachment {
        ensure_junction_contains_endpoint_member(network, &attachment.junction_id, &road.id, false)?;
    }
    for tee in &road.tee_attachments {
        validate_tee_attachment(network, road, tee)?;
    }
    Ok(())
}

fn validate_tee_attachment(
    network: &RoadNetwork,
    road: &Road,
    tee: &RoadTeeAttachment,
) -> Result<(), RoadError> {
    if tee.host_road_id != road.id {
        return Err(RoadError::AttachmentMismatch(format!(
            "road {} tee attachment host_road_id must reference itself",
            road.id
        )));
    }
    let host_road = network.roads.get(&tee.host_road_id).ok_or_else(|| {
        RoadError::InvalidJunctionReference(format!(
            "road {} tee references missing host road {}",
            road.id,
            tee.host_road_id
        ))
    })?;
    if host_road.id != road.id {
        return Err(RoadError::AttachmentMismatch(format!(
            "road {} tee host road mismatch",
            road.id
        )));
    }
    ensure_junction_contains_member(
        network,
        &tee.junction_id,
        &road.id,
        JunctionMemberRole::TeeHost,
        Some(tee.host_t),
    )?;
    Ok(())
}

fn ensure_junction_contains_endpoint_member(
    network: &RoadNetwork,
    junction_id: &JunctionId,
    road_id: &RoadId,
    is_start: bool,
) -> Result<(), RoadError> {
    let junction = network.junctions.get(junction_id).ok_or_else(|| {
        RoadError::InvalidJunctionReference(format!(
            "road {} references missing junction {}",
            road_id,
            junction_id
        ))
    })?;
    let expected_roles = if is_start {
        [JunctionMemberRole::EndpointStart, JunctionMemberRole::TeeBranch]
    } else {
        [JunctionMemberRole::EndpointEnd, JunctionMemberRole::TeeBranch]
    };
    let found = junction.members.iter().any(|member| {
        member.road_id == *road_id && expected_roles.contains(&member.role)
    });
    if !found {
        return Err(RoadError::AttachmentMismatch(format!(
            "road {} attachment to junction {} is not mirrored in junction members",
            road_id,
            junction_id
        )));
    }
    Ok(())
}

fn ensure_junction_contains_member(
    network: &RoadNetwork,
    junction_id: &JunctionId,
    road_id: &RoadId,
    role: JunctionMemberRole,
    host_t: Option<f32>,
) -> Result<(), RoadError> {
    let junction = network.junctions.get(junction_id).ok_or_else(|| {
        RoadError::InvalidJunctionReference(format!(
            "road {} references missing junction {}",
            road_id,
            junction_id
        ))
    })?;
    let found = junction.members.iter().any(|member| {
        member.road_id == *road_id
            && member.role == role
            && match (member.host_t, host_t) {
                (Some(lhs), Some(rhs)) => (lhs - rhs).abs() <= 1e-5,
                (None, None) => true,
                _ => false,
            }
    });
    if !found {
        return Err(RoadError::AttachmentMismatch(format!(
            "road {} attachment to junction {} is not mirrored in junction members",
            road_id,
            junction_id
        )));
    }
    Ok(())
}

fn validate_crossing(network: &RoadNetwork, crossing: &RoadCrossingOverride) -> Result<(), RoadError> {
    if crossing.road_a == crossing.road_b {
        return Err(RoadError::InvalidJunctionReference(
            "crossing override must reference two distinct roads".to_string(),
        ));
    }
    if !network.roads.contains_key(&crossing.road_a)
        || !network.roads.contains_key(&crossing.road_b)
    {
        return Err(RoadError::InvalidJunctionReference(
            "crossing override references missing road".to_string(),
        ));
    }
    for (road_id, t) in [(&crossing.road_a, crossing.road_a_t), (&crossing.road_b, crossing.road_b_t)] {
        if !t.is_finite() || !(0.0..=1.0).contains(&t) {
            return Err(RoadError::InvalidJunctionReference(format!(
                "crossing override for road {} has invalid t",
                road_id
            )));
        }
    }
    Ok(())
}
