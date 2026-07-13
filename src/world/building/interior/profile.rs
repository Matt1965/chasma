use bevy::prelude::*;

use super::catalog::{DoorTemplate, InteriorChildKind, InteriorChildPlacement, InteriorProfile};
use super::door::DoorAccessPolicy;
use super::door::DoorState;
use super::id::InteriorProfileId;
use crate::world::{
    BuildingDefinitionId, DoodadDefinitionId, PortalTemplate, PortalType, SpaceTemplate,
};

/// Two-story hut interior profile for B7 tests/dev.
pub fn two_story_hut_interior_profile() -> InteriorProfile {
    let spaces = vec![
        SpaceTemplate {
            key: "ground_interior",
            display_floor_label: "Ground Floor",
            visibility_group_id: 1,
            reference_elevation: 0.0,
            local_floor_y: 0.0,
            room_tag: Some("hall"),
        },
        SpaceTemplate {
            key: "upper_interior",
            display_floor_label: "Upper Floor",
            visibility_group_id: 2,
            reference_elevation: 4.0,
            local_floor_y: 4.0,
            room_tag: Some("bedroom"),
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
        PortalTemplate {
            key: "upper_hall_door",
            portal_type: PortalType::Doorway,
            from_space_key: "ground_interior",
            to_space_key: "upper_interior",
            from_local_xz: Vec2::new(1.0, 3.0),
            from_radius_meters: 1.0,
            to_local_position: Vec3::new(1.0, 4.0, 3.0),
            bidirectional: true,
        },
    ];
    let doors = vec![DoorTemplate {
        key: "upper_hall_door",
        portal_key: "upper_hall_door",
        initial_state: DoorState::Closed,
        access: DoorAccessPolicy::Everyone,
    }];
    let children = vec![
        InteriorChildPlacement {
            key: "ground_chair",
            kind: InteriorChildKind::Doodad(DoodadDefinitionId::new("interior_chair")),
            space_key: "ground_interior",
            local_position: Vec3::new(1.5, 0.0, 2.0),
            local_rotation: Quat::IDENTITY,
            enabled: true,
        },
        InteriorChildPlacement {
            key: "ground_workbench",
            kind: InteriorChildKind::Building(BuildingDefinitionId::new("workbench")),
            space_key: "ground_interior",
            local_position: Vec3::new(2.5, 0.0, 2.5),
            local_rotation: Quat::IDENTITY,
            enabled: true,
        },
    ];
    InteriorProfile::new(InteriorProfileId::new("two_story_hut"))
        .with_spaces(spaces)
        .with_portals(portals)
        .with_doors(doors)
        .with_children(children)
}
