//! Deterministic multi-region navigation blueprint fixtures (IN-07b).

use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationPolygon2d, NavigationRegionConnectionDefinition, NavigationRegionConnectionKind,
    NavigationRegionDefinition, NavigationVerticalTransitionDefinition,
    NavigationVerticalTransitionKind,
};

fn region(
    key: impl Into<String>,
    label: impl Into<String>,
    vertices: Vec<[f32; 2]>,
) -> NavigationRegionDefinition {
    NavigationRegionDefinition {
        key: key.into(),
        display_label: label.into(),
        room_tag: None,
        walkable_outline: NavigationPolygon2d {
            vertices_xz: vertices,
        },
    }
}

fn doorway(
    key: impl Into<String>,
    floor_key: impl Into<String>,
    from_region: impl Into<String>,
    to_region: impl Into<String>,
    from: [f32; 2],
    to: [f32; 2],
    radius: f32,
) -> NavigationRegionConnectionDefinition {
    NavigationRegionConnectionDefinition {
        key: key.into(),
        kind: NavigationRegionConnectionKind::Doorway,
        floor_key: floor_key.into(),
        from_region_key: from_region.into(),
        to_region_key: to_region.into(),
        from_local_position_xz: from,
        to_local_position_xz: to,
        radius_meters: radius,
        bidirectional: true,
        enabled: true,
        door_key: None,
    }
}

/// IN-11: one floor, one region, one doorless exterior entrance.
pub fn one_region_doorless_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("one_region_hut", "One Region Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![region(
                "main",
                "Main",
                vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]],
            )],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [4.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [4.0, 0.0, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}

/// Fixture 1: two-room hut with one doorway between rooms.
pub fn two_room_hut_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("two_room_hut", "Two Room Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![
                region(
                    "room_a",
                    "Room A",
                    vec![[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]],
                ),
                region(
                    "room_b",
                    "Room B",
                    vec![[6.4, 0.0], [12.4, 0.0], [12.4, 4.0], [6.4, 4.0]],
                ),
            ],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("room_a".to_string()),
            local_position_xz: [3.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [3.0, 0.0, 1.0],
            bidirectional: true,
            door_key: None,
        }])
        .with_region_connections(vec![doorway(
            "hall_door",
            "ground",
            "room_a",
            "room_b",
            [5.7, 2.0],
            [6.7, 2.0],
            0.8,
        )])
}

/// Fixture 2: room–corridor–room chain.
pub fn corridor_hut_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("corridor_hut", "Corridor Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![
                region(
                    "room_west",
                    "Room West",
                    vec![[0.0, 0.0], [5.0, 0.0], [5.0, 4.0], [0.0, 4.0]],
                ),
                region(
                    "corridor",
                    "Corridor",
                    vec![[5.4, 1.5], [10.6, 1.5], [10.6, 2.5], [5.4, 2.5]],
                ),
                region(
                    "room_east",
                    "Room East",
                    vec![[11.0, 0.0], [16.0, 0.0], [16.0, 4.0], [11.0, 4.0]],
                ),
            ],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("room_west".to_string()),
            local_position_xz: [2.5, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [2.5, 0.0, 1.0],
            bidirectional: true,
            door_key: None,
        }])
        .with_region_connections(vec![
            doorway(
                "west_door",
                "ground",
                "room_west",
                "corridor",
                [4.7, 2.0],
                [5.7, 2.0],
                0.7,
            ),
            doorway(
                "east_door",
                "ground",
                "corridor",
                "room_east",
                [10.3, 2.0],
                [11.3, 2.0],
                0.7,
            ),
        ])
}

/// Fixture 3: two-floor building with targeted regions and same-floor halls.
pub fn two_floor_two_room_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("two_floor_two_room", "Two Floor Two Room")
        .with_floors(vec![
            NavigationFloorDefinition {
                floor_id: 0,
                key: "ground".to_string(),
                display_label: "Ground".to_string(),
                elevation_meters: 0.0,
                visibility_group_id: 1,
                room_tag: None,
                walkable_outline_legacy: None,
                regions: vec![
                    region(
                        "ground_entry",
                        "Ground Entry",
                        vec![[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]],
                    ),
                    region(
                        "ground_back",
                        "Ground Back",
                        vec![[6.4, 0.0], [12.4, 0.0], [12.4, 6.0], [6.4, 6.0]],
                    ),
                ],
            },
            NavigationFloorDefinition {
                floor_id: 1,
                key: "upper".to_string(),
                display_label: "Upper".to_string(),
                elevation_meters: 4.0,
                visibility_group_id: 2,
                room_tag: None,
                walkable_outline_legacy: None,
                regions: vec![
                    region(
                        "upper_landing",
                        "Upper Landing",
                        vec![[6.4, 0.0], [12.4, 0.0], [12.4, 6.0], [6.4, 6.0]],
                    ),
                    region(
                        "upper_bed",
                        "Upper Bed",
                        vec![[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]],
                    ),
                ],
            },
        ])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("ground_entry".to_string()),
            local_position_xz: [3.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [3.0, 0.0, 1.0],
            bidirectional: true,
            door_key: None,
        }])
        .with_region_connections(vec![
            doorway(
                "ground_hall",
                "ground",
                "ground_entry",
                "ground_back",
                [5.7, 3.0],
                [6.7, 3.0],
                0.8,
            ),
            doorway(
                "upper_hall",
                "upper",
                "upper_landing",
                "upper_bed",
                [6.7, 3.0],
                [5.7, 3.0],
                0.8,
            ),
        ])
        .with_vertical_transitions(vec![NavigationVerticalTransitionDefinition {
            key: "stairs".to_string(),
            kind: NavigationVerticalTransitionKind::Stair,
            from_floor_key: "ground".to_string(),
            to_floor_key: "upper".to_string(),
            from_region_key: Some("ground_back".to_string()),
            to_region_key: Some("upper_landing".to_string()),
            from_local_position_xz: [9.0, 3.0],
            from_radius_meters: 1.25,
            to_local_position: [9.0, 4.0, 3.0],
            bidirectional: true,
        }])
}

/// Two doorway connections between the same region pair at different endpoints.
pub fn dual_doorway_navigation_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("dual_doorway", "Dual Doorway")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![
                region(
                    "west",
                    "West",
                    vec![[0.0, 0.0], [5.0, 0.0], [5.0, 8.0], [0.0, 8.0]],
                ),
                region(
                    "east",
                    "East",
                    vec![[5.4, 0.0], [10.4, 0.0], [10.4, 8.0], [5.4, 8.0]],
                ),
            ],
        }])
        .with_region_connections(vec![
            doorway(
                "door_north",
                "ground",
                "west",
                "east",
                [4.8, 6.0],
                [5.6, 6.0],
                0.7,
            ),
            doorway(
                "door_south",
                "ground",
                "west",
                "east",
                [4.8, 2.0],
                [5.6, 2.0],
                0.7,
            ),
        ])
}
