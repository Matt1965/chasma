//! Pending simulation-side trace events flushed after movement tick.

use bevy::prelude::*;

use crate::world::{
    CommandBufferResolveReport, CombatAiReport, CombatEngagementReport, CombatStrikeReport,
    ProjectileReport, UnitDeathReport, UnitMovementTrace,
};

/// Resolve report awaiting trace flush (simulation writes, debug reads).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PendingSimulationTrace {
    pub resolve: Option<CommandBufferResolveReport>,
    pub combat: Option<CombatEngagementReport>,
    pub combat_strike: Option<CombatStrikeReport>,
    pub projectile: Option<ProjectileReport>,
    pub death: Option<UnitDeathReport>,
    pub combat_ai: Option<CombatAiReport>,
    pub movement_traces: Vec<UnitMovementTrace>,
}
