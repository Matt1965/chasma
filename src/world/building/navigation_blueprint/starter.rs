//! Starter navigation blueprints aligned with B7 interior dev profiles.

use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationPolygon2d, NavigationVerticalTransitionDefinition, NavigationVerticalTransitionKind,
    single_region_floor,
};
use super::id::BuildingNavigationBlueprintId;

pub fn starter_navigation_blueprints() -> Vec<BuildingNavigationBlueprint> {
    vec![
        two_story_hut_navigation_blueprint(),
        barn_navigation_blueprint(),
    ]
}

/// Matches [`two_story_hut_interior_profile`] space/portal layout.
pub fn two_story_hut_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("two_story_hut", "Two Story Hut Navigation")
        .with_floors(vec![
            single_region_floor(
                0,
                "ground_interior",
                "Ground Floor",
                0.0,
                1,
                Some("hall".to_string()),
                NavigationPolygon2d::rectangle(4.0, 4.0),
            ),
            single_region_floor(
                1,
                "upper_interior",
                "Upper Floor",
                4.0,
                2,
                Some("bedroom".to_string()),
                NavigationPolygon2d::rectangle(4.0, 4.0),
            ),
        ])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground_interior".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [2.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [2.0, 0.0, 2.5],
            bidirectional: true,
            door_key: Some("exterior_entrance".to_string()),
        }])
        .with_vertical_transitions(vec![NavigationVerticalTransitionDefinition {
            key: "stairs".to_string(),
            kind: NavigationVerticalTransitionKind::Stair,
            from_floor_key: "ground_interior".to_string(),
            to_floor_key: "upper_interior".to_string(),
            from_region_key: Some("main".to_string()),
            to_region_key: Some("main".to_string()),
            from_local_position_xz: [1.5, 1.5],
            from_radius_meters: 1.25,
            to_local_position: [1.5, 4.0, 1.5],
            bidirectional: true,
        }])
}

/// Matches [`barn_interior_profile`] space/portal layout.
pub fn barn_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new(
        BuildingNavigationBlueprintId::new("barn_interior"),
        "Barn Navigation",
    )
    .with_floors(vec![single_region_floor(
        0,
        "barn_interior",
        "Barn Floor",
        0.0,
        1,
        Some("storage_hall".to_string()),
        NavigationPolygon2d::rectangle(8.0, 6.0),
    )])
    .with_entrances(vec![NavigationEntranceDefinition {
        key: "exterior_entrance".to_string(),
        floor_key: "barn_interior".to_string(),
        region_key: Some("main".to_string()),
        local_position_xz: [4.0, 0.0],
        radius_meters: 2.5,
        interior_spawn_local: [4.0, 0.0, 2.0],
        bidirectional: true,
        door_key: None,
    }])
}
