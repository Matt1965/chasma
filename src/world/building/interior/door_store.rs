use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::prelude::*;

use super::door::{DoorRecord, DoorState, portal_traversable_for_unit, unit_may_open_door};
use super::error::InteriorError;
use super::id::DoorId;
use crate::world::{BuildingId, BuildingOwnership, PortalId, SpaceId, UnitOwnership, WorldData};

/// Runtime door index on [`WorldData`] (ADR-084 B7).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct DoorStore {
    next_door_id: u32,
    doors: BTreeMap<DoorId, DoorRecord>,
    building_doors: HashMap<BuildingId, Vec<DoorId>>,
    portal_to_door: HashMap<PortalId, DoorId>,
}

impl DoorStore {
    pub fn allocate_door_id(&mut self) -> DoorId {
        let id = DoorId::new(self.next_door_id);
        self.next_door_id += 1;
        id
    }

    pub fn get(&self, id: DoorId) -> Option<&DoorRecord> {
        self.doors.get(&id)
    }

    pub fn get_mut(&mut self, id: DoorId) -> Option<&mut DoorRecord> {
        self.doors.get_mut(&id)
    }

    pub fn door_for_portal_id(&self, portal_id: PortalId) -> Option<DoorId> {
        self.portal_to_door.get(&portal_id).copied()
    }

    pub fn door_for_portal(&self, portal_id: PortalId) -> Option<&DoorRecord> {
        self.door_for_portal_id(portal_id)
            .and_then(|id| self.doors.get(&id))
    }

    pub fn building_door_ids(&self, building_id: BuildingId) -> &[DoorId] {
        self.building_doors
            .get(&building_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn insert_door(&mut self, record: DoorRecord) -> Result<(), InteriorError> {
        if self.doors.contains_key(&record.id) {
            return Err(InteriorError::DuplicateDoorId(record.id));
        }
        if self.portal_to_door.contains_key(&record.portal_id) {
            return Err(InteriorError::InvalidDoorPortal {
                door_key: record.definition_key.clone(),
                portal_key: record.portal_id.raw().to_string(),
            });
        }
        self.portal_to_door.insert(record.portal_id, record.id);
        self.building_doors
            .entry(record.owning_building_id)
            .or_default()
            .push(record.id);
        self.doors.insert(record.id, record);
        Ok(())
    }

    pub fn remove_building(&mut self, building_id: BuildingId) -> Vec<DoorId> {
        let ids = self.building_doors.remove(&building_id).unwrap_or_default();
        for id in &ids {
            if let Some(record) = self.doors.remove(id) {
                self.portal_to_door.remove(&record.portal_id);
            }
        }
        ids
    }

    pub fn next_id(&self) -> u32 {
        self.next_door_id
    }

    pub fn restore_next_id(&mut self, next: u32) {
        self.next_door_id = self.next_door_id.max(next);
    }

    /// Clear all doors (ADR-086 B9 scene load).
    pub fn clear(&mut self) {
        self.next_door_id = 1;
        self.doors.clear();
        self.building_doors.clear();
        self.portal_to_door.clear();
    }

    pub fn portal_enabled_for_door(state: DoorState) -> bool {
        state.portal_passable()
    }

    pub fn sync_portal_enabled(
        world: &mut WorldData,
        door_id: DoorId,
    ) -> Result<(), InteriorError> {
        let door = world
            .door_store()
            .get(door_id)
            .ok_or(InteriorError::DoorNotFound(door_id))?
            .clone();
        let enabled = Self::portal_enabled_for_door(door.state);
        if !world
            .space_registry_mut()
            .set_portal_enabled(door.portal_id, enabled)
        {
            return Err(InteriorError::PortalNotFound(door.portal_id));
        }
        Ok(())
    }
}

pub fn open_door(world: &mut WorldData, door_id: DoorId) -> Result<(), InteriorError> {
    let state = world
        .door_store()
        .get(door_id)
        .ok_or(InteriorError::DoorNotFound(door_id))?
        .state;
    if state == DoorState::Open {
        return Err(InteriorError::DoorAlreadyOpen(door_id));
    }
    if matches!(state, DoorState::Locked | DoorState::Destroyed) {
        return Err(InteriorError::UnauthorizedDoorAction { door_id });
    }
    world
        .door_store_mut()
        .get_mut(door_id)
        .ok_or(InteriorError::DoorNotFound(door_id))?
        .state = DoorState::Open;
    DoorStore::sync_portal_enabled(world, door_id)
}

pub fn close_door(world: &mut WorldData, door_id: DoorId) -> Result<(), InteriorError> {
    let state = world
        .door_store()
        .get(door_id)
        .ok_or(InteriorError::DoorNotFound(door_id))?
        .state;
    if state == DoorState::Closed {
        return Err(InteriorError::DoorAlreadyClosed(door_id));
    }
    if matches!(state, DoorState::Destroyed) {
        return Err(InteriorError::UnauthorizedDoorAction { door_id });
    }
    world
        .door_store_mut()
        .get_mut(door_id)
        .ok_or(InteriorError::DoorNotFound(door_id))?
        .state = DoorState::Closed;
    DoorStore::sync_portal_enabled(world, door_id)
}

pub fn lock_door(world: &mut WorldData, door_id: DoorId) -> Result<(), InteriorError> {
    world
        .door_store_mut()
        .get_mut(door_id)
        .ok_or(InteriorError::DoorNotFound(door_id))?
        .state = DoorState::Locked;
    DoorStore::sync_portal_enabled(world, door_id)
}

pub fn destroy_door(world: &mut WorldData, door_id: DoorId) -> Result<(), InteriorError> {
    world
        .door_store_mut()
        .get_mut(door_id)
        .ok_or(InteriorError::DoorNotFound(door_id))?
        .state = DoorState::Destroyed;
    DoorStore::sync_portal_enabled(world, door_id)
}

pub fn try_open_door_for_unit(
    world: &mut WorldData,
    door_id: DoorId,
    building_ownership: BuildingOwnership,
    unit_ownership: UnitOwnership,
) -> Result<bool, InteriorError> {
    let door = world
        .door_store()
        .get(door_id)
        .ok_or(InteriorError::DoorNotFound(door_id))?
        .clone();
    if door.state.portal_passable() {
        return Ok(false);
    }
    if !unit_may_open_door(&door, building_ownership, unit_ownership) {
        return Ok(false);
    }
    open_door(world, door_id)?;
    Ok(true)
}

pub fn portal_traversable(
    world: &WorldData,
    portal_id: PortalId,
    building_ownership: BuildingOwnership,
    unit_ownership: Option<UnitOwnership>,
) -> bool {
    let portal = match world.space_registry().get_portal(portal_id) {
        Some(portal) => portal,
        None => return false,
    };
    if portal.enabled {
        return true;
    }
    let door = world.door_store().door_for_portal(portal_id);
    let Some(unit) = unit_ownership else {
        return false;
    };
    portal_traversable_for_unit(door, building_ownership, unit)
}

pub fn try_open_door_at_portal_for_unit(
    world: &mut WorldData,
    portal_id: PortalId,
    building_ownership: BuildingOwnership,
    unit_ownership: UnitOwnership,
) -> Result<bool, InteriorError> {
    let Some(door_id) = world.door_store().door_for_portal_id(portal_id) else {
        return Ok(false);
    };
    try_open_door_for_unit(world, door_id, building_ownership, unit_ownership)
}

/// Collect portals from a space including door portals that are disabled but openable.
pub fn space_route_for_unit(
    world: &WorldData,
    from: SpaceId,
    to: SpaceId,
    unit_ownership: Option<UnitOwnership>,
) -> Option<Vec<PortalId>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut queue = std::collections::VecDeque::from([(from, Vec::<PortalId>::new())]);
    let mut visited = std::collections::BTreeSet::from([from]);
    while let Some((space, path)) = queue.pop_front() {
        for portal_id in world.space_registry().portals_from_space(space) {
            let portal_id = *portal_id;
            let portal = world.space_registry().get_portal(portal_id)?;
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
            let building_ownership = portal
                .owning_building_id
                .and_then(|id| world.get_building(id))
                .map(|record| record.ownership)
                .unwrap_or(BuildingOwnership::neutral());
            if !portal_traversable(world, portal_id, building_ownership, unit_ownership) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(portal_id);
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

/// Collect portals from a space including door portals that are disabled but openable.
pub fn traversable_portals_from_space(
    world: &WorldData,
    space: SpaceId,
    building_ownership: BuildingOwnership,
    unit_ownership: Option<UnitOwnership>,
) -> Vec<PortalId> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for portal_id in world.space_registry().portals_from_space(space) {
        let portal_id = *portal_id;
        let portal_building_ownership = world
            .space_registry()
            .get_portal(portal_id)
            .and_then(|portal| portal.owning_building_id)
            .and_then(|id| world.get_building(id))
            .map(|record| record.ownership)
            .unwrap_or(building_ownership);
        if seen.insert(portal_id)
            && portal_traversable(world, portal_id, portal_building_ownership, unit_ownership)
        {
            out.push(portal_id);
        }
    }
    out.sort();
    out
}
