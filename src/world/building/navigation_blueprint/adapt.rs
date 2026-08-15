//! Extension point: translate blueprints into runtime navigation templates (NV1.3 + NV2).

use bevy::prelude::*;

use super::definition::{
    BuildingNavigationBlueprint, NavigationRegionConnectionKind, NavigationVerticalTransitionKind,
};
use crate::world::PortalType;

/// Runtime space key for one region: `{floor_key}/{region_key}`.
pub fn region_space_key(floor_key: &str, region_key: &str) -> String {
    format!("{floor_key}/{region_key}")
}

/// Floor key prefix from a qualified region space key.
pub fn floor_key_from_region_space_key(space_key: &str) -> &str {
    space_key
        .split_once('/')
        .map(|(floor, _)| floor)
        .unwrap_or(space_key)
}

/// Owned space template derived from a navigation blueprint region.
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintSpaceTemplate {
    pub key: String,
    pub display_floor_label: String,
    pub visibility_group_id: u32,
    pub reference_elevation: f32,
    pub local_floor_y: f32,
    pub room_tag: Option<String>,
}

/// Owned portal template derived from blueprint entrances, connections, and vertical transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintPortalTemplate {
    pub key: String,
    pub portal_type: PortalType,
    pub from_space_key: String,
    pub to_space_key: String,
    pub from_local_xz: Vec2,
    pub from_radius_meters: f32,
    pub to_local_position: Vec3,
    pub bidirectional: bool,
    pub enabled: bool,
    /// Owning region polygon edge index (exterior entrances only).
    pub entrance_owning_edge_index: Option<u32>,
    /// Boundary threshold on the owning edge in blueprint-local XZ (exterior entrances only).
    pub entrance_threshold_local_xz: Option<Vec2>,
}

fn resolve_region_space_key(
    blueprint: &BuildingNavigationBlueprint,
    floor_key: &str,
    region_key: Option<&str>,
    feature_key: &str,
) -> String {
    let region = blueprint
        .resolve_region_key(floor_key, region_key, feature_key)
        .unwrap_or_else(|err| {
            panic!(
                "blueprint `{}` feature `{}` region resolution failed: {err}",
                blueprint.id.as_str(),
                feature_key
            )
        });
    region_space_key(floor_key, region)
}

fn floor_elevation(blueprint: &BuildingNavigationBlueprint, floor_key: &str) -> f32 {
    blueprint
        .floor_by_key(floor_key)
        .map(|floor| floor.elevation_meters)
        .unwrap_or_else(|| panic!("blueprint floor `{floor_key}` missing"))
}

/// Convert blueprint regions to owned space templates for registration.
pub fn blueprint_space_templates(
    blueprint: &BuildingNavigationBlueprint,
) -> Vec<BlueprintSpaceTemplate> {
    blueprint
        .floors
        .iter()
        .flat_map(|floor| {
            floor.regions.iter().map(|region| BlueprintSpaceTemplate {
                key: region_space_key(&floor.key, &region.key),
                display_floor_label: floor.display_label.clone(),
                visibility_group_id: floor.visibility_group_id,
                reference_elevation: floor.elevation_meters,
                local_floor_y: floor.elevation_meters,
                room_tag: region.room_tag.clone().or_else(|| floor.room_tag.clone()),
            })
        })
        .collect()
}

/// Convert blueprint features to owned portal templates for registration.
pub fn blueprint_portal_templates(
    blueprint: &BuildingNavigationBlueprint,
) -> Vec<BlueprintPortalTemplate> {
    let mut portals = Vec::new();
    for entrance in &blueprint.entrances {
        let region_key = match blueprint.resolve_region_key(
            &entrance.floor_key,
            entrance.region_key.as_deref(),
            &entrance.key,
        ) {
            Ok(key) => key,
            Err(_) => continue,
        };
        let Some(floor) = blueprint.floor_by_key(&entrance.floor_key) else {
            continue;
        };
        let Some(region) = floor.region_by_key(region_key) else {
            continue;
        };
        let threshold = Vec2::new(entrance.local_position_xz[0], entrance.local_position_xz[1]);
        let projection = super::entrance_geometry::project_point_to_boundary(
            &region.walkable_outline.vertices_xz,
            threshold,
            super::entrance_geometry::ENTRANCE_CORNER_MARGIN,
        )
        .unwrap_or_else(|| {
            panic!(
                "entrance `{}` on blueprint `{}` is not anchored to region `{}` boundary",
                entrance.key,
                blueprint.id.as_str(),
                region.key
            )
        });
        let exterior = super::entrance_geometry::derive_exterior_staging_xz(
            projection.point,
            projection.outward_normal,
            super::entrance_geometry::DEFAULT_EXTERIOR_STAGING_OFFSET,
        );
        portals.push(BlueprintPortalTemplate {
            key: entrance.key.clone(),
            portal_type: PortalType::ExteriorEntrance,
            from_space_key: "surface".to_string(),
            to_space_key: resolve_region_space_key(
                blueprint,
                &entrance.floor_key,
                entrance.region_key.as_deref(),
                &entrance.key,
            ),
            from_local_xz: exterior,
            from_radius_meters: entrance.radius_meters,
            to_local_position: Vec3::new(
                entrance.interior_spawn_local[0],
                entrance.interior_spawn_local[1],
                entrance.interior_spawn_local[2],
            ),
            bidirectional: entrance.bidirectional,
            enabled: true,
            entrance_owning_edge_index: Some(projection.edge_index as u32),
            entrance_threshold_local_xz: Some(projection.point),
        });
    }
    for connection in &blueprint.region_connections {
        let elevation = floor_elevation(blueprint, &connection.floor_key);
        portals.push(BlueprintPortalTemplate {
            key: connection.key.clone(),
            portal_type: PortalType::Doorway,
            from_space_key: region_space_key(&connection.floor_key, &connection.from_region_key),
            to_space_key: region_space_key(&connection.floor_key, &connection.to_region_key),
            from_local_xz: Vec2::new(
                connection.from_local_position_xz[0],
                connection.from_local_position_xz[1],
            ),
            from_radius_meters: connection.radius_meters,
            to_local_position: Vec3::new(
                connection.to_local_position_xz[0],
                elevation,
                connection.to_local_position_xz[1],
            ),
            bidirectional: connection.bidirectional,
            enabled: connection.enabled,
            entrance_owning_edge_index: None,
            entrance_threshold_local_xz: None,
        });
        if connection.kind == NavigationRegionConnectionKind::OpenArch {
            // OpenArch uses the same portal type; door state is governed by `enabled` only.
        }
    }
    for transition in &blueprint.vertical_transitions {
        portals.push(BlueprintPortalTemplate {
            key: transition.key.clone(),
            portal_type: match transition.kind {
                NavigationVerticalTransitionKind::Stair => PortalType::Stair,
                NavigationVerticalTransitionKind::Ramp => PortalType::Ramp,
                NavigationVerticalTransitionKind::Ladder => PortalType::Stair,
            },
            from_space_key: resolve_region_space_key(
                blueprint,
                &transition.from_floor_key,
                transition.from_region_key.as_deref(),
                &transition.key,
            ),
            to_space_key: resolve_region_space_key(
                blueprint,
                &transition.to_floor_key,
                transition.to_region_key.as_deref(),
                &transition.key,
            ),
            from_local_xz: Vec2::new(
                transition.from_local_position_xz[0],
                transition.from_local_position_xz[1],
            ),
            from_radius_meters: transition.from_radius_meters,
            to_local_position: Vec3::new(
                transition.to_local_position[0],
                transition.to_local_position[1],
                transition.to_local_position[2],
            ),
            bidirectional: transition.bidirectional,
            enabled: true,
            entrance_owning_edge_index: None,
            entrance_threshold_local_xz: None,
        });
    }
    portals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::fixtures::{
        corridor_hut_navigation_blueprint, dual_doorway_navigation_blueprint,
        two_floor_two_room_navigation_blueprint, two_room_hut_navigation_blueprint,
    };
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;

    #[test]
    fn single_region_floor_gets_bare_key_alias_at_registration() {
        let blueprint = two_story_hut_navigation_blueprint();
        let spaces = blueprint_space_templates(&blueprint);
        assert_eq!(spaces.len(), 2);
        assert!(spaces.iter().any(|s| s.key == "ground_interior/main"));
        assert!(spaces.iter().any(|s| s.key == "upper_interior/main"));
    }

    #[test]
    fn multi_region_spaces_use_qualified_keys_only() {
        let blueprint = two_room_hut_navigation_blueprint();
        let spaces = blueprint_space_templates(&blueprint);
        assert_eq!(spaces.len(), 2);
        assert!(spaces.iter().any(|s| s.key == "ground/room_a"));
        assert!(spaces.iter().any(|s| s.key == "ground/room_b"));
        assert!(!spaces.iter().any(|s| s.key == "ground"));
    }

    #[test]
    fn region_connection_portal_templates_use_doorway_type() {
        let blueprint = two_room_hut_navigation_blueprint();
        let portals = blueprint_portal_templates(&blueprint);
        let hall = portals
            .iter()
            .find(|p| p.key == "hall_door")
            .expect("hall_door");
        assert_eq!(hall.portal_type, PortalType::Doorway);
        assert_eq!(hall.from_space_key, "ground/room_a");
        assert_eq!(hall.to_space_key, "ground/room_b");
        assert!(hall.enabled);
    }

    #[test]
    fn entrance_targets_qualified_region_space() {
        let blueprint = two_room_hut_navigation_blueprint();
        let portals = blueprint_portal_templates(&blueprint);
        let entrance = portals
            .iter()
            .find(|p| p.key == "exterior_entrance")
            .expect("entrance");
        assert_eq!(entrance.to_space_key, "ground/room_a");
    }

    #[test]
    fn vertical_transition_targets_qualified_regions() {
        let blueprint = two_floor_two_room_navigation_blueprint();
        let portals = blueprint_portal_templates(&blueprint);
        let stairs = portals.iter().find(|p| p.key == "stairs").expect("stairs");
        assert_eq!(stairs.from_space_key, "ground/ground_back");
        assert_eq!(stairs.to_space_key, "upper/upper_landing");
    }

    #[test]
    fn corridor_fixture_registers_two_connection_portals() {
        let blueprint = corridor_hut_navigation_blueprint();
        let portals = blueprint_portal_templates(&blueprint);
        assert!(portals.iter().any(|p| p.key == "west_door"));
        assert!(portals.iter().any(|p| p.key == "east_door"));
    }

    #[test]
    fn dual_doorway_fixture_registers_both_connections() {
        let blueprint = dual_doorway_navigation_blueprint();
        let portals = blueprint_portal_templates(&blueprint);
        let keys: Vec<_> = portals.iter().map(|p| p.key.as_str()).collect();
        assert!(keys.contains(&"door_north"));
        assert!(keys.contains(&"door_south"));
    }
}
