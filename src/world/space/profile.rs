use bevy::prelude::*;

use super::definition::SpaceRecord;
use super::id::{PortalId, SpaceId};
use super::portal::{PortalRecord, PortalType};
use super::registry::SpaceRegistry;
use crate::world::{BuildingRecord, ChunkLayout, LocalPosition, WorldPosition};

/// Authoring template for a building-local space (ADR-083 B6, ADR-084 B7).
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceTemplate {
    pub key: &'static str,
    pub display_floor_label: &'static str,
    pub visibility_group_id: u32,
    pub reference_elevation: f32,
    pub local_floor_y: f32,
    /// Optional room/zone tag (metadata only in B7).
    pub room_tag: Option<&'static str>,
}

/// Authoring template for a portal relative to building anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct PortalTemplate {
    pub key: &'static str,
    pub portal_type: PortalType,
    pub from_space_key: &'static str,
    pub to_space_key: &'static str,
    pub from_local_xz: Vec2,
    pub from_radius_meters: f32,
    pub to_local_position: Vec3,
    pub bidirectional: bool,
}

/// Two-story hut profile for B6 tests/dev.
pub fn two_story_hut_profile() -> (Vec<SpaceTemplate>, Vec<PortalTemplate>) {
    let spaces = vec![
        SpaceTemplate {
            key: "ground_interior",
            display_floor_label: "Ground Floor",
            visibility_group_id: 1,
            reference_elevation: 0.0,
            local_floor_y: 0.0,
            room_tag: None,
        },
        SpaceTemplate {
            key: "upper_interior",
            display_floor_label: "Upper Floor",
            visibility_group_id: 2,
            reference_elevation: 4.0,
            local_floor_y: 4.0,
            room_tag: None,
        },
    ];
    let portals = vec![
        PortalTemplate {
            key: "exterior_entrance",
            portal_type: PortalType::ExteriorEntrance,
            from_space_key: "surface",
            to_space_key: "ground_interior",
            from_local_xz: Vec2::new(2.0, 0.0),
            from_radius_meters: 1.5,
            to_local_position: Vec3::new(2.0, 0.0, 1.0),
            bidirectional: true,
        },
        PortalTemplate {
            key: "stairs",
            portal_type: PortalType::Stair,
            from_space_key: "ground_interior",
            to_space_key: "upper_interior",
            from_local_xz: Vec2::new(3.0, 3.0),
            from_radius_meters: 1.25,
            to_local_position: Vec3::new(3.0, 4.0, 3.0),
            bidirectional: true,
        },
    ];
    (spaces, portals)
}

/// Instantiate profile spaces/portals for a placed building.
pub fn register_building_space_profile(
    registry: &mut SpaceRegistry,
    building: &BuildingRecord,
    layout: ChunkLayout,
    spaces: &[SpaceTemplate],
    portals: &[PortalTemplate],
) -> (
    std::collections::BTreeMap<String, SpaceId>,
    std::collections::BTreeMap<String, PortalId>,
) {
    let anchor_global = building.placement.position.to_global(layout);
    let rotation = building.placement.rotation;

    let mut key_to_space: std::collections::BTreeMap<&str, SpaceId> =
        std::collections::BTreeMap::from([("surface", SpaceId::SURFACE)]);

    let mut space_records = Vec::new();
    for template in spaces {
        let id = registry.allocate_space_id();
        key_to_space.insert(template.key, id);
        let floor_offset = rotation * Vec3::new(0.0, template.local_floor_y, 0.0);
        space_records.push(SpaceRecord {
            id,
            owning_building_id: Some(building.id),
            display_floor_label: template.display_floor_label.to_string(),
            visibility_group_id: template.visibility_group_id,
            reference_elevation: template.reference_elevation,
            floor_y_global: anchor_global.y + floor_offset.y,
            room_tag: template.room_tag.map(str::to_string),
            enabled: true,
            walkable: true,
        });
    }

    let mut portal_records = Vec::new();
    let mut portal_key_to_id: std::collections::BTreeMap<&str, super::id::PortalId> =
        std::collections::BTreeMap::new();
    for template in portals {
        let from_space = *key_to_space
            .get(template.from_space_key)
            .expect("from space key");
        let to_space = *key_to_space
            .get(template.to_space_key)
            .expect("to space key");
        let from_global = anchor_global
            + rotation * Vec3::new(template.from_local_xz.x, 0.0, template.from_local_xz.y);
        let to_global = anchor_global + rotation * template.to_local_position;
        let portal_id = registry.allocate_portal_id();
        portal_key_to_id.insert(template.key, portal_id);
        portal_records.push(PortalRecord {
            id: portal_id,
            portal_type: template.portal_type,
            from_space,
            to_space,
            from_center_global_xz: Vec2::new(from_global.x, from_global.z),
            from_radius_meters: template.from_radius_meters,
            to_position: WorldPosition::from_global(to_global, layout),
            traversal_cost: 1.0,
            bidirectional: template.bidirectional,
            enabled: true,
            owning_building_id: Some(building.id),
        });
    }

    registry.register_building_spaces(building.id, space_records, portal_records);
    (
        key_to_space
            .into_iter()
            .map(|(key, id)| (key.to_string(), id))
            .collect(),
        portal_key_to_id
            .into_iter()
            .map(|(key, id)| (key.to_string(), id))
            .collect(),
    )
}

/// Whether a space should be visible given active view context.
pub fn space_visible_in_view(
    active_space: SpaceId,
    active_group: u32,
    candidate: &SpaceRecord,
) -> bool {
    if candidate.id == active_space {
        return true;
    }
    candidate.visibility_group_id == active_group && active_group != 0
}

/// Hide spaces above active reference elevation by default.
pub fn space_hidden_by_default(active: &SpaceRecord, candidate: &SpaceRecord) -> bool {
    if candidate.id == active.id {
        return false;
    }
    candidate.reference_elevation > active.reference_elevation + 0.01
}
