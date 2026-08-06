use bevy::prelude::*;

use super::id::{PortalId, SpaceId};
use super::registry::SpaceRegistry;
use super::support::ground_position_in_space;
use crate::world::{ChunkLayout, WorldData, WorldPosition};

/// Portal traversal kind (ADR-083 B6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum PortalType {
    Stair,
    Ramp,
    ExteriorEntrance,
    Doorway,
    CaveEntrance,
}

impl PortalType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stair => "Stair",
            Self::Ramp => "Ramp",
            Self::ExteriorEntrance => "ExteriorEntrance",
            Self::Doorway => "Doorway",
            Self::CaveEntrance => "CaveEntrance",
        }
    }
}

/// Authoritative portal instance connecting two spaces (ADR-083 B6).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct PortalRecord {
    pub id: PortalId,
    pub portal_type: PortalType,
    pub from_space: SpaceId,
    pub to_space: SpaceId,
    /// Transition region center in global XZ (A-side / forward entry trigger).
    pub from_center_global_xz: Vec2,
    pub from_radius_meters: f32,
    /// Destination spawn position after forward (A → B) transition.
    pub to_position: WorldPosition,
    pub traversal_cost: f32,
    pub bidirectional: bool,
    pub enabled: bool,
    pub owning_building_id: Option<crate::world::BuildingId>,
}

impl PortalRecord {
    /// Whether traversal may begin from `current_space`.
    pub fn can_traverse_from(&self, current_space: SpaceId) -> bool {
        if !self.enabled {
            return false;
        }
        if self.from_space == current_space {
            true
        } else {
            self.bidirectional && self.to_space == current_space
        }
    }

    /// Global XZ center of the entry trigger while the agent is in `current_space`.
    ///
    /// Trigger geometry is independent of [`Self::enabled`]; traversal still requires a passable
    /// portal (door open or doorless).
    pub fn trigger_center_xz_for_space(
        &self,
        current_space: SpaceId,
        layout: ChunkLayout,
    ) -> Option<Vec2> {
        if self.from_space == current_space {
            Some(self.from_center_global_xz)
        } else if self.bidirectional && self.to_space == current_space {
            let global = self.to_position.to_global(layout);
            Some(Vec2::new(global.x, global.z))
        } else {
            None
        }
    }

    /// Whether `agent_global_xz` lies inside the entry trigger for `current_space`.
    pub fn contains_agent_in_space(
        &self,
        agent_global_xz: Vec2,
        current_space: SpaceId,
        layout: ChunkLayout,
    ) -> bool {
        let Some(center) = self.trigger_center_xz_for_space(current_space, layout) else {
            return false;
        };
        if !(self.from_radius_meters > 0.0) || !self.from_radius_meters.is_finite() {
            return false;
        }
        agent_global_xz.distance(center) <= self.from_radius_meters
    }

    /// Forward-only trigger test (A-side geometry).
    pub fn contains_agent_global(&self, agent_global_xz: Vec2) -> bool {
        if !(self.from_radius_meters > 0.0) || !self.from_radius_meters.is_finite() {
            return false;
        }
        agent_global_xz.distance(self.from_center_global_xz) <= self.from_radius_meters
    }

    /// Grounded world position of the entry trigger in `current_space`.
    pub fn trigger_world_position_in_space(
        &self,
        current_space: SpaceId,
        layout: ChunkLayout,
        world: &WorldData,
        space_registry: &SpaceRegistry,
    ) -> Option<WorldPosition> {
        let center_xz = self.trigger_center_xz_for_space(current_space, layout)?;
        let global = Vec3::new(center_xz.x, 0.0, center_xz.y);
        let position = WorldPosition::from_global(global, layout);
        ground_position_in_space(world, space_registry, current_space, position)
    }

    /// Destination space and grounded position after traversing from `current_space`.
    pub fn destination_for_traversal(
        &self,
        current_space: SpaceId,
        layout: ChunkLayout,
        world: &WorldData,
        space_registry: &SpaceRegistry,
    ) -> Option<(SpaceId, WorldPosition)> {
        if !self.can_traverse_from(current_space) {
            return None;
        }
        self.destination_geometry(current_space, layout, world, space_registry)
    }

    /// Destination geometry for path planning (ignores [`Self::enabled`]).
    pub fn destination_for_planning(
        &self,
        current_space: SpaceId,
        layout: ChunkLayout,
        world: &WorldData,
        space_registry: &SpaceRegistry,
    ) -> Option<(SpaceId, WorldPosition)> {
        if self.from_space == current_space {
            self.destination_geometry(current_space, layout, world, space_registry)
        } else if self.bidirectional && self.to_space == current_space {
            self.destination_geometry(current_space, layout, world, space_registry)
        } else {
            None
        }
    }

    fn destination_geometry(
        &self,
        current_space: SpaceId,
        layout: ChunkLayout,
        world: &WorldData,
        space_registry: &SpaceRegistry,
    ) -> Option<(SpaceId, WorldPosition)> {
        if self.from_space == current_space {
            let dest_space = self.to_space;
            let grounded =
                ground_position_in_space(world, space_registry, dest_space, self.to_position)?;
            Some((dest_space, grounded))
        } else {
            let dest_space = self.from_space;
            let global = Vec3::new(
                self.from_center_global_xz.x,
                0.0,
                self.from_center_global_xz.y,
            );
            let position = WorldPosition::from_global(global, layout);
            let grounded = ground_position_in_space(world, space_registry, dest_space, position)?;
            Some((dest_space, grounded))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, SpaceRecord,
        SpaceRegistry, WorldData,
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

    fn entrance_portal(interior: SpaceId) -> PortalRecord {
        PortalRecord {
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
        }
    }

    fn registry_with_interior() -> (SpaceRegistry, SpaceId) {
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
        (registry, interior)
    }

    #[test]
    fn forward_side_resolves_exterior_trigger_and_interior_destination() {
        let world = flat_world();
        let (mut registry, interior) = registry_with_interior();
        let portal = entrance_portal(interior);
        let layout = layout();

        assert_eq!(
            portal.trigger_center_xz_for_space(SpaceId::SURFACE, layout),
            Some(Vec2::new(10.0, 10.0))
        );
        let (_, dest) = portal
            .destination_for_traversal(SpaceId::SURFACE, layout, &world, &registry)
            .unwrap();
        assert_eq!(dest, pos(10.0, 0.0, 11.0));
    }

    #[test]
    fn reverse_side_resolves_interior_trigger_and_exterior_destination() {
        let world = flat_world();
        let (mut registry, interior) = registry_with_interior();
        let portal = entrance_portal(interior);
        let layout = layout();

        assert_eq!(
            portal.trigger_center_xz_for_space(interior, layout),
            Some(Vec2::new(10.0, 11.0))
        );
        let (dest_space, dest) = portal
            .destination_for_traversal(interior, layout, &world, &registry)
            .unwrap();
        assert_eq!(dest_space, SpaceId::SURFACE);
        assert!((dest.to_global(layout).x - 10.0).abs() < 1e-4);
        assert!((dest.to_global(layout).z - 10.0).abs() < 1e-4);
    }

    #[test]
    fn invalid_space_cannot_traverse() {
        let world = flat_world();
        let (mut registry, interior) = registry_with_interior();
        let mut portal = entrance_portal(interior);
        portal.bidirectional = false;
        let layout = layout();
        let other = registry.allocate_space_id();

        assert!(!portal.can_traverse_from(other));
        assert!(
            portal
                .destination_for_traversal(other, layout, &world, &registry)
                .is_none()
        );
    }

    #[test]
    fn non_bidirectional_rejects_reverse_traversal() {
        let world = flat_world();
        let (registry, interior) = registry_with_interior();
        let mut portal = entrance_portal(interior);
        portal.bidirectional = false;
        let layout = layout();

        assert!(!portal.can_traverse_from(interior));
        assert!(
            portal
                .destination_for_traversal(interior, layout, &world, &registry)
                .is_none()
        );
    }

    #[test]
    fn disabled_portal_rejects_traversal() {
        let world = flat_world();
        let (registry, interior) = registry_with_interior();
        let mut portal = entrance_portal(interior);
        portal.enabled = false;
        let layout = layout();

        assert!(!portal.can_traverse_from(SpaceId::SURFACE));
        assert!(
            portal
                .destination_for_traversal(SpaceId::SURFACE, layout, &world, &registry)
                .is_none()
        );
    }

    #[test]
    fn disabled_portal_preserves_trigger_geometry() {
        let (_registry, interior) = registry_with_interior();
        let mut portal = entrance_portal(interior);
        portal.enabled = false;
        let layout = layout();

        assert_eq!(
            portal.trigger_center_xz_for_space(SpaceId::SURFACE, layout),
            Some(Vec2::new(10.0, 10.0))
        );
        assert!(portal.contains_agent_in_space(Vec2::new(10.0, 10.0), SpaceId::SURFACE, layout));
    }

    fn stair_portal(ground: SpaceId, upper: SpaceId) -> PortalRecord {
        PortalRecord {
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
        }
    }

    fn registry_with_two_floors() -> (SpaceRegistry, SpaceId, SpaceId) {
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
        (registry, ground, upper)
    }

    #[test]
    fn stair_forward_side_uses_ground_trigger_and_upper_destination() {
        let world = flat_world();
        let (registry, ground, upper) = registry_with_two_floors();
        let portal = stair_portal(ground, upper);
        let layout = layout();

        assert_eq!(
            portal.trigger_center_xz_for_space(ground, layout),
            Some(Vec2::new(83.0, 83.0))
        );
        let (dest_space, dest) = portal
            .destination_for_traversal(ground, layout, &world, &registry)
            .unwrap();
        assert_eq!(dest_space, upper);
        assert!((dest.to_global(layout).y - 4.0).abs() < 0.05);
    }

    #[test]
    fn stair_reverse_side_uses_upper_trigger_and_ground_destination() {
        let world = flat_world();
        let (registry, ground, upper) = registry_with_two_floors();
        let portal = stair_portal(ground, upper);
        let layout = layout();

        assert_eq!(
            portal.trigger_center_xz_for_space(upper, layout),
            Some(Vec2::new(83.0, 83.0))
        );
        let (dest_space, dest) = portal
            .destination_for_traversal(upper, layout, &world, &registry)
            .unwrap();
        assert_eq!(dest_space, ground);
        assert!(dest.to_global(layout).y.abs() < 0.05);
    }
}
