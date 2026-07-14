use bevy::prelude::*;

use crate::world::unit::UnitRecord;
use crate::world::{BuildingOwnership, UnitId};

/// Container access policy on building definitions (ADR-091 I5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum ContainerAccessPolicy {
    Everyone,
    #[default]
    OwnerOnly,
    Team,
}

impl ContainerAccessPolicy {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "owner" | "owneronly" | "owner_only" => Ok(Self::OwnerOnly),
            "everyone" | "public" | "all" => Ok(Self::Everyone),
            "team" | "party" => Ok(Self::Team),
            other => Err(format!("unknown container access policy `{other}`")),
        }
    }

    pub fn allows(self, building: BuildingOwnership, unit: &UnitRecord, locked: bool) -> bool {
        if locked {
            return false;
        }
        match self {
            Self::Everyone => true,
            Self::OwnerOnly => building
                .owner_id
                .map_or(true, |owner| unit.owner_id == Some(owner)),
            Self::Team => {
                building.team_id.is_some() && unit.team_id == building.team_id
                    || building
                        .owner_id
                        .map_or(false, |owner| unit.owner_id == Some(owner))
            }
        }
    }
}

/// Why inventory access was denied (ADR-091 I5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryAccessDenialReason {
    RequesterMissing(UnitId),
    InventoryMissing,
    BuildingNotFound(crate::world::BuildingId),
    BuildingHasNoInventory,
    BuildingNotOperational,
    ContainerLocked,
    PolicyDenied,
    WrongSpace,
    OutOfRange,
}

/// Result of [`super::inventory::can_unit_access_building_inventory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryAccessResult {
    Allowed,
    Denied(InventoryAccessDenialReason),
}

impl InventoryAccessResult {
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}
