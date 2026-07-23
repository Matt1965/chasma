//! Starter navigation blueprints aligned with B7 interior dev profiles.

use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationPolygon2d, NavigationVerticalTransitionDefinition,
    NavigationVerticalTransitionKind,
};
use super::id::BuildingNavigationBlueprintId;

pub fn starter_navigation_blueprints() -> Vec<BuildingNavigationBlueprint> {
    vec![two_story_hut_navigation_blueprint(), barn_navigation_blueprint()]
}

/// Matches [`two_story_hut_interior_profile`] space/portal layout.
pub fn two_story_hut_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("two_story_hut", "Two Story Hut Navigation")
        .with_floors(vec![
            NavigationFloorDefinition {
                floor_id: 0,
                key: "ground_interior".to_string(),
                display_label: "Ground Floor".to_string(),
                elevation_meters: 0.0,
                visibility_group_id: 1,
                room_tag: Some("hall".to_string()),
                walkable_outline: NavigationPolygon2d::rectangle(4.0, 4.0),
            },
            NavigationFloorDefinition {
                floor_id: 1,
                key: "upper_interior".to_string(),
                display_label: "Upper Floor".to_string(),
                elevation_meters: 4.0,
                visibility_group_id: 2,
                room_tag: Some("bedroom".to_string()),
                walkable_outline: NavigationPolygon2d::rectangle(4.0, 4.0),
            },
        ])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground_interior".to_string(),
            local_position_xz: [2.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [2.0, 0.0, 1.0],
            bidirectional: true,
        }])
        .with_vertical_transitions(vec![NavigationVerticalTransitionDefinition {
            key: "stairs".to_string(),
            kind: NavigationVerticalTransitionKind::Stair,
            from_floor_key: "ground_interior".to_string(),
            to_floor_key: "upper_interior".to_string(),
            from_local_position_xz: [3.0, 3.0],
            from_radius_meters: 1.25,
            to_local_position: [3.0, 4.0, 3.0],
            bidirectional: true,
        }])
}

/// Matches [`barn_interior_profile`] space/portal layout.
pub fn barn_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new(
        BuildingNavigationBlueprintId::new("barn_interior"),
        "Barn Navigation",
    )
    .with_floors(vec![NavigationFloorDefinition {
        floor_id: 0,
        key: "barn_interior".to_string(),
        display_label: "Barn Floor".to_string(),
        elevation_meters: 0.0,
        visibility_group_id: 1,
        room_tag: Some("storage_hall".to_string()),
        walkable_outline: NavigationPolygon2d::rectangle(8.0, 6.0),
    }])
    .with_entrances(vec![NavigationEntranceDefinition {
        key: "exterior_entrance".to_string(),
        floor_key: "barn_interior".to_string(),
        local_position_xz: [4.0, 0.0],
        radius_meters: 2.5,
        interior_spawn_local: [4.0, 0.0, 2.0],
        bidirectional: true,
    }])
}
