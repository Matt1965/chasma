use bevy::prelude::*;

use super::id::ItemPileId;
use crate::world::inventory::ItemInstanceId;
use crate::world::ownership::{Affiliation, OwnerId, TeamId};
use crate::world::{ItemDefinitionId, SpaceId, WorldPosition};

/// How a pile is authored (ADR-090 I4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum ItemPileSource {
    #[default]
    Dropped,
    Spilled,
    DevSpawned,
}

/// Single authoritative entry on a world pile — no grid (ADR-090 I4).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum WorldPileContents {
    Stack {
        item_definition_id: ItemDefinitionId,
        quantity: u32,
    },
    Unique {
        item_instance_id: ItemInstanceId,
    },
}

/// Authoritative world item pile (ADR-090 I4).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct WorldItemPileRecord {
    pub id: ItemPileId,
    pub placement: WorldPosition,
    pub current_space_id: SpaceId,
    pub contents: WorldPileContents,
    pub owner_id: Option<OwnerId>,
    pub team_id: Option<TeamId>,
    pub affiliation: Affiliation,
    pub source: ItemPileSource,
    pub created_tick: u64,
}

impl WorldItemPileRecord {
    pub fn new_stack(
        id: ItemPileId,
        placement: WorldPosition,
        current_space_id: SpaceId,
        item_definition_id: ItemDefinitionId,
        quantity: u32,
        owner_id: Option<OwnerId>,
        team_id: Option<TeamId>,
        affiliation: Affiliation,
        source: ItemPileSource,
        created_tick: u64,
    ) -> Self {
        Self {
            id,
            placement,
            current_space_id,
            contents: WorldPileContents::Stack {
                item_definition_id,
                quantity,
            },
            owner_id,
            team_id,
            affiliation,
            source,
            created_tick,
        }
    }

    pub fn new_unique(
        id: ItemPileId,
        placement: WorldPosition,
        current_space_id: SpaceId,
        item_instance_id: ItemInstanceId,
        owner_id: Option<OwnerId>,
        team_id: Option<TeamId>,
        affiliation: Affiliation,
        source: ItemPileSource,
        created_tick: u64,
    ) -> Self {
        Self {
            id,
            placement,
            current_space_id,
            contents: WorldPileContents::Unique { item_instance_id },
            owner_id,
            team_id,
            affiliation,
            source,
            created_tick,
        }
    }

    pub fn stack_quantity(&self) -> Option<u32> {
        match &self.contents {
            WorldPileContents::Stack { quantity, .. } => Some(*quantity),
            WorldPileContents::Unique { .. } => None,
        }
    }
}
