use bevy::prelude::*;

use super::id::DoorId;
use crate::world::{BuildingId, BuildingOwnership, OwnerId, PortalId, TeamId, UnitOwnership};

/// Authoritative door state (ADR-084 B7). Presentation follows this; not animation timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum DoorState {
    #[default]
    Closed,
    Open,
    Locked,
    Destroyed,
}

impl DoorState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Closed => "Closed",
            Self::Open => "Open",
            Self::Locked => "Locked",
            Self::Destroyed => "Destroyed",
        }
    }

    pub fn portal_passable(self) -> bool {
        matches!(self, Self::Open | Self::Destroyed)
    }
}

/// Runtime access policy for a door (ADR-084 B7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum DoorAccessPolicy {
    #[default]
    Everyone,
    OwnerOnly,
    Team,
    Locked,
}

impl DoorAccessPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Everyone => "Everyone",
            Self::OwnerOnly => "OwnerOnly",
            Self::Team => "Team",
            Self::Locked => "Locked",
        }
    }
}

/// One authoritative door bound to a portal edge (ADR-084 B7).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct DoorRecord {
    pub id: DoorId,
    pub owning_building_id: BuildingId,
    pub portal_id: PortalId,
    pub definition_key: String,
    pub state: DoorState,
    pub access: DoorAccessPolicy,
}

/// Whether a unit may open or traverse a closed door.
pub fn unit_may_open_door(
    door: &DoorRecord,
    building_ownership: BuildingOwnership,
    unit_ownership: UnitOwnership,
) -> bool {
    if door.state == DoorState::Locked || door.access == DoorAccessPolicy::Locked {
        return false;
    }
    if door.state == DoorState::Destroyed || door.state == DoorState::Open {
        return true;
    }
    match door.access {
        DoorAccessPolicy::Everyone => true,
        DoorAccessPolicy::OwnerOnly => same_owner(building_ownership, unit_ownership),
        DoorAccessPolicy::Team => same_team(building_ownership, unit_ownership),
        DoorAccessPolicy::Locked => false,
    }
}

pub fn portal_traversable_for_unit(
    door: Option<&DoorRecord>,
    building_ownership: BuildingOwnership,
    unit_ownership: UnitOwnership,
) -> bool {
    let Some(door) = door else {
        return true;
    };
    if door.state.portal_passable() {
        return true;
    }
    unit_may_open_door(door, building_ownership, unit_ownership)
}

fn same_owner(building: BuildingOwnership, unit: UnitOwnership) -> bool {
    match (building.owner_id, unit.owner_id) {
        (Some(a), Some(b)) => a == b,
        _ => {
            building.affiliation == unit.affiliation
                && building.affiliation == crate::world::Affiliation::Player
        }
    }
}

fn same_team(building: BuildingOwnership, unit: UnitOwnership) -> bool {
    match (building.team_id, unit.team_id) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Affiliation;

    fn door(access: DoorAccessPolicy, state: DoorState) -> DoorRecord {
        DoorRecord {
            id: DoorId::new(1),
            owning_building_id: BuildingId::new(1),
            portal_id: PortalId::new(1),
            definition_key: "test".into(),
            state,
            access,
        }
    }

    #[test]
    fn open_and_destroyed_are_passable() {
        assert!(
            door(DoorAccessPolicy::Everyone, DoorState::Open)
                .state
                .portal_passable()
        );
        assert!(
            door(DoorAccessPolicy::Everyone, DoorState::Destroyed)
                .state
                .portal_passable()
        );
        assert!(
            !door(DoorAccessPolicy::Everyone, DoorState::Closed)
                .state
                .portal_passable()
        );
    }

    #[test]
    fn locked_door_denies_open() {
        let building = BuildingOwnership::neutral();
        let unit = UnitOwnership::neutral();
        assert!(!unit_may_open_door(
            &door(DoorAccessPolicy::Locked, DoorState::Locked),
            building,
            unit
        ));
    }

    #[test]
    fn owner_only_requires_matching_owner() {
        let building = BuildingOwnership {
            owner_id: Some(OwnerId::new(7)),
            team_id: None,
            affiliation: Affiliation::Player,
        };
        let allowed = UnitOwnership {
            owner_id: Some(OwnerId::new(7)),
            team_id: None,
            affiliation: Affiliation::Player,
        };
        let denied = UnitOwnership::neutral();
        let record = door(DoorAccessPolicy::OwnerOnly, DoorState::Closed);
        assert!(unit_may_open_door(&record, building, allowed));
        assert!(!unit_may_open_door(&record, building, denied));
    }
}
