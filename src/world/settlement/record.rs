//! Settlement and treasury records (ADR-093 I7).

use bevy::prelude::*;

use super::anchor::SettlementAnchorId;
use super::id::{SettlementId, TreasuryId};
use crate::world::{Affiliation, OwnerId, TeamId, WorldPosition};

/// Faction ownership for a settlement (mirrors building ownership seam).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct SettlementOwnership {
    pub owner_id: Option<OwnerId>,
    pub team_id: Option<TeamId>,
    pub affiliation: Affiliation,
}

impl SettlementOwnership {
    pub fn player_default() -> Self {
        let ownership = crate::world::UnitOwnership::player_default();
        Self {
            owner_id: ownership.owner_id,
            team_id: ownership.team_id,
            affiliation: ownership.affiliation,
        }
    }

    pub fn from_building_ownership(ownership: crate::world::BuildingOwnership) -> Self {
        Self {
            owner_id: ownership.owner_id,
            team_id: ownership.team_id,
            affiliation: ownership.affiliation,
        }
    }
}

/// Authoritative settlement identity (ADR-093 I7, ADR-133).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SettlementRecord {
    pub id: SettlementId,
    pub display_name: String,
    pub treasury_id: TreasuryId,
    pub anchor_id: SettlementAnchorId,
    pub center: WorldPosition,
    pub boundary_radius_meters: f32,
    pub ownership: SettlementOwnership,
    /// Treasury / interaction point (defaults to center at creation).
    pub interaction_position: WorldPosition,
    pub created_tick: u64,
}

/// Abstract settlement wealth — never stores inventory ids (ADR-093 I7).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SettlementTreasuryRecord {
    pub id: TreasuryId,
    pub settlement_id: SettlementId,
    pub ownership: SettlementOwnership,
    /// Abstract gold balance (coins deposited from physical items).
    pub balance_gold: u64,
    pub created_tick: u64,
    pub metadata: String,
}

/// Dev/audit log entry for treasury mutations (ADR-093 I7).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct TreasuryTransactionRecord {
    pub tick: u64,
    pub treasury_id: TreasuryId,
    pub settlement_id: SettlementId,
    pub unit_id: Option<crate::world::UnitId>,
    pub source_inventory_id: Option<crate::world::InventoryId>,
    pub deposited_gold: u32,
    pub balance_after: u64,
}
