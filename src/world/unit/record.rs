use bevy::prelude::*;

use super::catalog::UnitDefinitionId;
use super::id::UnitId;
use super::metadata::UnitMetadata;
use super::placement::UnitPlacement;
use super::source::UnitSource;
use super::state::UnitState;
use crate::world::ownership::{Affiliation, OwnerId, TeamId, UnitOwnership};

/// One authoritative unit instance (ADR-027 U2, ADR-051 O1).
///
/// [`UnitDefinitionId`] is the authoritative type reference. Instance records
/// do **not** copy catalog `faction_tag` as ownership; runtime owner/team/
/// affiliation live here.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct UnitRecord {
    pub id: UnitId,
    pub definition_id: UnitDefinitionId,
    pub placement: UnitPlacement,
    pub state: UnitState,
    pub source: UnitSource,
    pub metadata: UnitMetadata,
    /// Direct controller — not derived from catalog `faction_tag`.
    pub owner_id: Option<OwnerId>,
    /// Ally/enemy grouping for future combat/diplomacy.
    pub team_id: Option<TeamId>,
    /// Broad classification for UI and controllability.
    pub affiliation: Affiliation,
}

impl UnitRecord {
    pub fn new(
        id: UnitId,
        definition_id: UnitDefinitionId,
        placement: UnitPlacement,
        source: UnitSource,
        ownership: UnitOwnership,
    ) -> Self {
        Self {
            id,
            definition_id,
            placement,
            state: UnitState::default(),
            source,
            metadata: UnitMetadata,
            owner_id: ownership.owner_id,
            team_id: ownership.team_id,
            affiliation: ownership.affiliation,
        }
    }

    pub fn ownership(&self) -> UnitOwnership {
        UnitOwnership {
            owner_id: self.owner_id,
            team_id: self.team_id,
            affiliation: self.affiliation,
        }
    }
}
