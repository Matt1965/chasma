use bevy::prelude::*;

use crate::world::{Affiliation, OwnerId, TeamId, UnitOwnership};

/// Authoritative building ownership assigned at spawn (B2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct BuildingOwnership {
    pub owner_id: Option<OwnerId>,
    pub team_id: Option<TeamId>,
    pub affiliation: Affiliation,
}

impl BuildingOwnership {
    pub fn from_unit_ownership(ownership: UnitOwnership) -> Self {
        Self {
            owner_id: ownership.owner_id,
            team_id: ownership.team_id,
            affiliation: ownership.affiliation,
        }
    }

    pub fn with_affiliation(affiliation: Affiliation) -> Self {
        Self::from_unit_ownership(UnitOwnership::with_affiliation(affiliation))
    }

    pub fn neutral() -> Self {
        Self::from_unit_ownership(UnitOwnership::neutral())
    }
}

impl Default for BuildingOwnership {
    fn default() -> Self {
        Self::neutral()
    }
}
