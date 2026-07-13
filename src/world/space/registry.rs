use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;

use super::definition::SpaceRecord;
use super::id::{PortalId, SpaceId};
use super::portal::PortalRecord;
use crate::world::BuildingId;

/// Authoritative space and portal graph (ADR-083 B6).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct SpaceRegistry {
    next_space_id: u32,
    next_portal_id: u32,
    spaces: BTreeMap<SpaceId, SpaceRecord>,
    portals: BTreeMap<PortalId, PortalRecord>,
    building_spaces: HashMap<BuildingId, Vec<SpaceId>>,
    portals_from_space: HashMap<SpaceId, Vec<PortalId>>,
}

impl SpaceRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry
            .spaces
            .insert(SpaceId::SURFACE, SpaceRecord::surface());
        registry.next_space_id = 1;
        registry
    }

    pub fn allocate_space_id(&mut self) -> SpaceId {
        let id = SpaceId::new(self.next_space_id);
        self.next_space_id += 1;
        id
    }

    pub fn allocate_portal_id(&mut self) -> PortalId {
        let id = PortalId::new(self.next_portal_id);
        self.next_portal_id += 1;
        id
    }

    pub fn get_space(&self, id: SpaceId) -> Option<&SpaceRecord> {
        self.spaces.get(&id)
    }

    pub fn get_portal(&self, id: PortalId) -> Option<&PortalRecord> {
        self.portals.get(&id)
    }

    pub fn spaces(&self) -> impl Iterator<Item = (&SpaceId, &SpaceRecord)> {
        self.spaces.iter()
    }

    pub fn portals(&self) -> impl Iterator<Item = (&PortalId, &PortalRecord)> {
        self.portals.iter()
    }

    pub fn portals_from_space(&self, space: SpaceId) -> &[PortalId] {
        self.portals_from_space
            .get(&space)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn building_space_ids(&self, building_id: BuildingId) -> &[SpaceId] {
        self.building_spaces
            .get(&building_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn insert_space(&mut self, record: SpaceRecord) {
        self.spaces.insert(record.id, record);
    }

    pub fn insert_portal(&mut self, record: PortalRecord) {
        self.portals_from_space
            .entry(record.from_space)
            .or_default()
            .push(record.id);
        if record.bidirectional {
            self.portals_from_space
                .entry(record.to_space)
                .or_default()
                .push(record.id);
        }
        self.portals.insert(record.id, record);
    }

    pub fn register_building_spaces(
        &mut self,
        building_id: BuildingId,
        spaces: impl IntoIterator<Item = SpaceRecord>,
        portals: impl IntoIterator<Item = PortalRecord>,
    ) {
        let mut ids = Vec::new();
        for space in spaces {
            ids.push(space.id);
            self.insert_space(space);
        }
        self.building_spaces.insert(building_id, ids);
        for portal in portals {
            self.insert_portal(portal);
        }
    }

    pub fn remove_building(&mut self, building_id: BuildingId) {
        if let Some(space_ids) = self.building_spaces.remove(&building_id) {
            for space_id in space_ids {
                self.spaces.remove(&space_id);
                self.portals_from_space.remove(&space_id);
            }
        }
        let removed_portals: Vec<PortalId> = self
            .portals
            .iter()
            .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
            .map(|(id, _)| *id)
            .collect();
        for portal_id in removed_portals {
            self.portals.remove(&portal_id);
        }
        for ids in self.portals_from_space.values_mut() {
            ids.retain(|id| self.portals.contains_key(id));
        }
    }

    pub fn set_portal_enabled(&mut self, portal_id: PortalId, enabled: bool) -> bool {
        if let Some(portal) = self.portals.get_mut(&portal_id) {
            portal.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Deterministic portal list for a space, sorted by portal id.
    pub fn sorted_portals_from_space(&self, space: SpaceId) -> Vec<&PortalRecord> {
        let mut portals: Vec<_> = self
            .portals_from_space(space)
            .iter()
            .filter_map(|id| self.portals.get(id))
            .filter(|portal| portal.enabled)
            .collect();
        portals.sort_by_key(|portal| portal.id);
        portals
    }

    /// BFS space connectivity for cross-space path planning.
    pub fn space_route(&self, from: SpaceId, to: SpaceId) -> Option<Vec<PortalId>> {
        if from == to {
            return Some(Vec::new());
        }
        let mut queue = std::collections::VecDeque::from([(from, Vec::<PortalId>::new())]);
        let mut visited = std::collections::BTreeSet::from([from]);
        while let Some((space, path)) = queue.pop_front() {
            for portal in self.sorted_portals_from_space(space) {
                let next = if portal.from_space == space {
                    portal.to_space
                } else if portal.bidirectional && portal.to_space == space {
                    portal.from_space
                } else {
                    continue;
                };
                if !visited.insert(next) {
                    continue;
                }
                let mut next_path = path.clone();
                next_path.push(portal.id);
                if next == to {
                    return Some(next_path);
                }
                if next_path.len() >= 8 {
                    continue;
                }
                queue.push_back((next, next_path));
            }
        }
        None
    }

    pub fn next_space_id(&self) -> u32 {
        self.next_space_id
    }

    pub fn next_portal_id(&self) -> u32 {
        self.next_portal_id
    }

    pub fn restore_next_ids(&mut self, next_space: u32, next_portal: u32) {
        self.next_space_id = self.next_space_id.max(next_space);
        self.next_portal_id = self.next_portal_id.max(next_portal);
    }

    /// Reset to surface-only registry (ADR-086 B9 scene load).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition, WorldPosition};

    fn portal_between(id: PortalId, from: SpaceId, to: SpaceId, x: f32, z: f32) -> PortalRecord {
        PortalRecord {
            id,
            portal_type: super::super::portal::PortalType::Stair,
            from_space: from,
            to_space: to,
            from_center_global_xz: Vec2::new(x, z),
            from_radius_meters: 1.5,
            to_position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(x, 4.0, z)),
            ),
            traversal_cost: 1.0,
            bidirectional: true,
            enabled: true,
            owning_building_id: None,
        }
    }

    #[test]
    fn space_route_finds_portal_chain() {
        let mut registry = SpaceRegistry::new();
        let upper = registry.allocate_space_id();
        let ground = registry.allocate_space_id();
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
        registry.insert_portal(portal_between(
            PortalId::new(1),
            SpaceId::SURFACE,
            ground,
            10.0,
            10.0,
        ));
        registry.insert_portal(portal_between(PortalId::new(2), ground, upper, 10.0, 10.0));
        let route = registry.space_route(SpaceId::SURFACE, upper).unwrap();
        assert_eq!(route.len(), 2);
    }
}
