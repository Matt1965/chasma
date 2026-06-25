//! Pending simulation-side trace events flushed after movement tick.

use bevy::prelude::*;

use crate::world::{CommandBufferResolveReport, CombatEngagementReport, CombatStrikeReport, UnitDeathReport};

/// Resolve report awaiting trace flush (simulation writes, debug reads).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PendingSimulationTrace {
    pub resolve: Option<CommandBufferResolveReport>,
    pub combat: Option<CombatEngagementReport>,
    pub combat_strike: Option<CombatStrikeReport>,
    pub death: Option<UnitDeathReport>,
}
