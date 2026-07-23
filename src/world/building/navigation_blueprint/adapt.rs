//! Extension point: translate blueprints into runtime navigation templates (NV1.3).
//!
//! Consumed by interior activation when a navigation blueprint catalog is available.

use bevy::prelude::*;

use super::definition::{
    BuildingNavigationBlueprint, NavigationVerticalTransitionKind,
};
use crate::world::PortalType;

/// Owned space template derived from a navigation blueprint floor.
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintSpaceTemplate {
    pub key: String,
    pub display_floor_label: String,
    pub visibility_group_id: u32,
    pub reference_elevation: f32,
    pub local_floor_y: f32,
    pub room_tag: Option<String>,
}

/// Owned portal template derived from blueprint entrances and vertical transitions.
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
}

/// Convert blueprint floors to owned space templates for future registration.
pub fn blueprint_space_templates(
    blueprint: &BuildingNavigationBlueprint,
) -> Vec<BlueprintSpaceTemplate> {
    blueprint
        .floors
        .iter()
        .map(|floor| BlueprintSpaceTemplate {
            key: floor.key.clone(),
            display_floor_label: floor.display_label.clone(),
            visibility_group_id: floor.visibility_group_id,
            reference_elevation: floor.elevation_meters,
            local_floor_y: floor.elevation_meters,
            room_tag: floor.room_tag.clone(),
        })
        .collect()
}

/// Convert blueprint entrances and vertical transitions to owned portal templates.
pub fn blueprint_portal_templates(
    blueprint: &BuildingNavigationBlueprint,
) -> Vec<BlueprintPortalTemplate> {
    let mut portals = Vec::new();
    for entrance in &blueprint.entrances {
        portals.push(BlueprintPortalTemplate {
            key: entrance.key.clone(),
            portal_type: PortalType::ExteriorEntrance,
            from_space_key: "surface".to_string(),
            to_space_key: entrance.floor_key.clone(),
            from_local_xz: Vec2::new(entrance.local_position_xz[0], entrance.local_position_xz[1]),
            from_radius_meters: entrance.radius_meters,
            to_local_position: Vec3::new(
                entrance.interior_spawn_local[0],
                entrance.interior_spawn_local[1],
                entrance.interior_spawn_local[2],
            ),
            bidirectional: entrance.bidirectional,
        });
    }
    for transition in &blueprint.vertical_transitions {
        portals.push(BlueprintPortalTemplate {
            key: transition.key.clone(),
            portal_type: match transition.kind {
                NavigationVerticalTransitionKind::Stair => PortalType::Stair,
                NavigationVerticalTransitionKind::Ramp => PortalType::Ramp,
                NavigationVerticalTransitionKind::Ladder => PortalType::Stair,
            },
            from_space_key: transition.from_floor_key.clone(),
            to_space_key: transition.to_floor_key.clone(),
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
        });
    }
    portals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;

    #[test]
    fn blueprint_adapts_to_owned_templates() {
        let blueprint = two_story_hut_navigation_blueprint();
        assert_eq!(blueprint_space_templates(&blueprint).len(), 2);
        assert_eq!(blueprint_portal_templates(&blueprint).len(), 2);
    }
}
