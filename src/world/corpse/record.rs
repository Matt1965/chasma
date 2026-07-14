use bevy::prelude::*;

use super::id::CorpseId;
use crate::world::SpaceId;
use crate::world::inventory::InventoryId;
use crate::world::ownership::{Affiliation, OwnerId, TeamId};
use crate::world::unit::{UnitDefinitionId, UnitId, UnitPlacement};

/// Authoritative corpse lifecycle state (ADR-089 I3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum CorpseState {
    #[default]
    Present,
    Expired,
}

/// Authoritative corpse container (ADR-089 I3).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct CorpseRecord {
    pub id: CorpseId,
    pub origin_unit_id: UnitId,
    pub unit_definition_id: UnitDefinitionId,
    pub placement: UnitPlacement,
    pub current_space_id: SpaceId,
    pub inventory_id: Option<InventoryId>,
    pub owner_id: Option<OwnerId>,
    pub team_id: Option<TeamId>,
    pub affiliation: Affiliation,
    pub created_tick: u64,
    pub remaining_lifetime_ticks: u64,
    pub state: CorpseState,
}

impl CorpseRecord {
    pub fn new(
        id: CorpseId,
        origin_unit_id: UnitId,
        unit_definition_id: UnitDefinitionId,
        placement: UnitPlacement,
        current_space_id: SpaceId,
        inventory_id: Option<InventoryId>,
        owner_id: Option<OwnerId>,
        team_id: Option<TeamId>,
        affiliation: Affiliation,
        created_tick: u64,
        lifetime_ticks: u64,
    ) -> Self {
        Self {
            id,
            origin_unit_id,
            unit_definition_id,
            placement,
            current_space_id,
            inventory_id,
            owner_id,
            team_id,
            affiliation,
            created_tick,
            remaining_lifetime_ticks: lifetime_ticks,
            state: CorpseState::Present,
        }
    }
}
