use bevy::prelude::*;

use super::id::{PortalId, SpaceId};
use super::portal::PortalRecord;
use super::registry::SpaceRegistry;
use crate::world::{ChunkLayout, WorldData, WorldPosition};

/// Per-unit portal hysteresis to prevent oscillation (ADR-083 B6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitPortalTransitionState {
    pub lockout_portal: Option<PortalId>,
}

/// Authoritative portal transition when agent enters a portal region.
pub fn try_portal_transition(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    current_space: SpaceId,
    agent_position: WorldPosition,
    transition_state: &mut UnitPortalTransitionState,
    preferred_portal: Option<PortalId>,
) -> Option<(SpaceId, WorldPosition, PortalId)> {
    let agent_global = agent_position.to_global(layout);
    let agent_xz = Vec2::new(agent_global.x, agent_global.z);

    let mut candidates: Vec<&PortalRecord> = space_registry
        .sorted_portals_from_space(current_space)
        .into_iter()
        .filter(|portal| portal.can_traverse_from(current_space))
        .collect();

    if let Some(preferred) = preferred_portal {
        candidates.sort_by_key(|portal| if portal.id == preferred { 0 } else { 1 });
    }

    for portal in candidates {
        if transition_state.lockout_portal == Some(portal.id) {
            if !portal.contains_agent_in_space(agent_xz, current_space, layout) {
                transition_state.lockout_portal = None;
            } else {
                continue;
            }
        }

        if !portal.contains_agent_in_space(agent_xz, current_space, layout) {
            continue;
        }

        let (dest_space, dest_position) =
            portal.destination_for_traversal(current_space, layout, world, space_registry)?;

        transition_state.lockout_portal = Some(portal.id);
        return Some((dest_space, dest_position, portal.id));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, PortalType,
        SpaceRecord, SpaceRegistry, WorldData,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, y: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, y, z)),
        )
    }

    fn setup_entrance() -> (WorldData, SpaceRegistry, SpaceId, PortalRecord) {
        let world = flat_world();
        let mut registry = SpaceRegistry::new();
        let interior = registry.allocate_space_id();
        registry.insert_space(SpaceRecord {
            id: interior,
            owning_building_id: None,
            display_floor_label: "Ground".into(),
            visibility_group_id: 1,
            reference_elevation: 0.0,
            floor_y_global: 0.0,
            room_tag: None,
            enabled: true,
            walkable: true,
        });
        let portal = PortalRecord {
            id: PortalId::new(1),
            portal_type: PortalType::ExteriorEntrance,
            from_space: SpaceId::SURFACE,
            to_space: interior,
            from_center_global_xz: Vec2::new(10.0, 10.0),
            from_radius_meters: 1.5,
            to_position: pos(10.0, 0.0, 11.0),
            traversal_cost: 1.0,
            bidirectional: true,
            enabled: true,
            owning_building_id: None,
        };
        registry.insert_portal(portal.clone());
        (world, registry, interior, portal)
    }

    #[test]
    fn forward_unit_in_trigger_transitions_to_interior() {
        let (world, registry, interior, _) = setup_entrance();
        let agent = pos(10.0, 0.0, 10.0);
        let mut state = UnitPortalTransitionState::default();
        let result = try_portal_transition(
            &world,
            &registry,
            layout(),
            SpaceId::SURFACE,
            agent,
            &mut state,
            None,
        )
        .unwrap();
        assert_eq!(result.0, interior);
        assert_eq!(result.2, PortalId::new(1));
    }

    #[test]
    fn reverse_unit_in_interior_trigger_transitions_to_surface() {
        let (world, registry, interior, _) = setup_entrance();
        let agent = pos(10.0, 0.0, 11.0);
        let mut state = UnitPortalTransitionState::default();
        let result = try_portal_transition(
            &world,
            &registry,
            layout(),
            interior,
            agent,
            &mut state,
            None,
        )
        .unwrap();
        assert_eq!(result.0, SpaceId::SURFACE);
    }

    #[test]
    fn reverse_unit_in_forward_trigger_does_not_transition() {
        let (world, registry, interior, _) = setup_entrance();
        // Forward trigger XZ overlaps reverse trigger radius; stay outside B-side trigger.
        let agent = pos(10.0, 0.0, 8.0);
        let mut state = UnitPortalTransitionState::default();
        assert!(
            try_portal_transition(
                &world,
                &registry,
                layout(),
                interior,
                agent,
                &mut state,
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn lockout_prevents_immediate_reversal() {
        let (world, registry, interior, _) = setup_entrance();
        let agent = pos(10.0, 0.0, 10.0);
        let mut state = UnitPortalTransitionState::default();
        let first = try_portal_transition(
            &world,
            &registry,
            layout(),
            SpaceId::SURFACE,
            agent,
            &mut state,
            None,
        );
        assert!(first.is_some());
        let interior_agent = pos(10.0, 0.0, 11.0);
        let second = try_portal_transition(
            &world,
            &registry,
            layout(),
            interior,
            interior_agent,
            &mut state,
            None,
        );
        assert!(second.is_none());
    }

    #[test]
    fn lockout_clears_after_leaving_destination_trigger() {
        let (world, registry, interior, portal) = setup_entrance();
        let agent = pos(10.0, 0.0, 10.0);
        let mut state = UnitPortalTransitionState::default();
        let _ = try_portal_transition(
            &world,
            &registry,
            layout(),
            SpaceId::SURFACE,
            agent,
            &mut state,
            None,
        );
        let interior_agent = pos(10.0, 0.0, 11.0);
        assert!(
            try_portal_transition(
                &world,
                &registry,
                layout(),
                interior,
                interior_agent,
                &mut state,
                None,
            )
            .is_none()
        );
        let away = pos(20.0, 0.0, 20.0);
        let _ = try_portal_transition(
            &world,
            &registry,
            layout(),
            interior,
            away,
            &mut state,
            Some(portal.id),
        );
        let back_at_trigger = pos(10.0, 0.0, 11.0);
        assert!(
            try_portal_transition(
                &world,
                &registry,
                layout(),
                interior,
                back_at_trigger,
                &mut state,
                None,
            )
            .is_some()
        );
    }

    fn setup_stair() -> (WorldData, SpaceRegistry, SpaceId, SpaceId, PortalRecord) {
        let world = flat_world();
        let mut registry = SpaceRegistry::new();
        let ground = registry.allocate_space_id();
        let upper = registry.allocate_space_id();
        registry.insert_space(SpaceRecord {
            id: ground,
            owning_building_id: None,
            display_floor_label: "Ground".into(),
            visibility_group_id: 1,
            reference_elevation: 0.0,
            floor_y_global: 0.0,
            room_tag: None,
            enabled: true,
            walkable: true,
        });
        registry.insert_space(SpaceRecord {
            id: upper,
            owning_building_id: None,
            display_floor_label: "Upper".into(),
            visibility_group_id: 2,
            reference_elevation: 4.0,
            floor_y_global: 4.0,
            room_tag: None,
            enabled: true,
            walkable: true,
        });
        let portal = PortalRecord {
            id: PortalId::new(2),
            portal_type: PortalType::Stair,
            from_space: ground,
            to_space: upper,
            from_center_global_xz: Vec2::new(83.0, 83.0),
            from_radius_meters: 1.25,
            to_position: pos(83.0, 4.0, 83.0),
            traversal_cost: 1.0,
            bidirectional: true,
            enabled: true,
            owning_building_id: None,
        };
        registry.insert_portal(portal.clone());
        (world, registry, ground, upper, portal)
    }

    #[test]
    fn stair_ground_to_upper_transition() {
        let (world, registry, ground, upper, portal) = setup_stair();
        let agent = pos(83.0, 0.0, 83.0);
        let mut state = UnitPortalTransitionState::default();
        let result = try_portal_transition(
            &world,
            &registry,
            layout(),
            ground,
            agent,
            &mut state,
            Some(portal.id),
        )
        .unwrap();
        assert_eq!(result.0, upper);
        assert!((result.1.to_global(layout()).y - 4.0).abs() < 0.05);
    }

    #[test]
    fn stair_upper_to_ground_transition() {
        let (world, registry, ground, upper, portal) = setup_stair();
        let agent = pos(83.0, 4.0, 83.0);
        let mut state = UnitPortalTransitionState::default();
        let result = try_portal_transition(
            &world,
            &registry,
            layout(),
            upper,
            agent,
            &mut state,
            Some(portal.id),
        )
        .unwrap();
        assert_eq!(result.0, ground);
        assert!(result.1.to_global(layout()).y.abs() < 0.05);
    }
}
