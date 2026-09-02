//! Settlement anchor records (ADR-133).

use bevy::prelude::*;

use super::id::SettlementAnchorId;
use crate::world::WorldPosition;
use crate::world::settlement::SettlementId;

/// Dedicated world object anchoring settlement identity and spatial center (ADR-133).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SettlementAnchorRecord {
    pub id: SettlementAnchorId,
    pub settlement_id: SettlementId,
    pub position: WorldPosition,
    pub created_tick: u64,
}
