use bevy::prelude::*;

use super::id::{DoorId, InteriorProfileId};
use crate::world::{BuildingId, DoodadDefinitionId, PortalId, SpaceId};

/// Structured interior authoring/runtime errors (ADR-084 B7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteriorError {
    MissingInteriorProfile(InteriorProfileId),
    MissingSpace {
        profile: InteriorProfileId,
        key: String,
    },
    MissingPortal {
        profile: InteriorProfileId,
        key: String,
    },
    MissingDoorDefinition {
        profile: InteriorProfileId,
        key: String,
    },
    InvalidDoorPortal {
        door_key: String,
        portal_key: String,
    },
    DuplicateDoorId(DoorId),
    UnauthorizedDoorAction {
        door_id: DoorId,
    },
    DoorAlreadyOpen(DoorId),
    DoorAlreadyClosed(DoorId),
    MissingChildDefinition {
        key: String,
        definition: String,
    },
    InvalidChildPlacement {
        key: String,
        reason: String,
    },
    ParentBuildingMissing(BuildingId),
    OrphanedInteriorObject {
        building_id: BuildingId,
    },
    InteriorSpawnFailed {
        building_id: BuildingId,
        reason: String,
    },
    BuildingInteriorAlreadyActive(BuildingId),
    DoorNotFound(DoorId),
    PortalNotFound(PortalId),
    SpaceUnavailable(SpaceId),
}

impl std::fmt::Display for InteriorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for InteriorError {}

impl InteriorError {
    pub fn missing_child_definition(key: &str, id: &DoodadDefinitionId) -> Self {
        Self::MissingChildDefinition {
            key: key.to_string(),
            definition: id.as_str().to_string(),
        }
    }
}
