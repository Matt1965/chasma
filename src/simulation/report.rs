//! Aggregated authoritative simulation tick reports (ADR-065).

use crate::world::{
    BatchUnitMovementReport, CombatAiReport, CombatEngagementReport, CombatStrikeReport,
    CommandBufferResolveReport, ProjectileReport, UnitDeathReport,
};

/// Outcome of one authoritative simulation tick (all stages).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimulationTickReport {
    pub movement: BatchUnitMovementReport,
    pub command_resolve: CommandBufferResolveReport,
    pub combat: CombatEngagementReport,
    pub combat_strike: CombatStrikeReport,
    pub projectile: ProjectileReport,
    pub death: UnitDeathReport,
    pub combat_ai: CombatAiReport,
}

impl SimulationTickReport {
    pub fn orders_resolved(&self) -> u32 {
        self.command_resolve.resolved
    }

    pub fn units_moved(&self) -> u32 {
        self.movement.moved
    }

    pub fn units_arrived(&self) -> u32 {
        self.movement.arrived
    }

    pub fn units_removed(&self) -> u32 {
        self.death.removed_unit_ids.len() as u32
    }

    pub fn movement_blocked_total(&self) -> u32 {
        let m = &self.movement;
        m.blocked_terrain_unavailable
            + m.blocked_slope_unavailable
            + m.blocked_slope_too_steep
            + m.blocked_by_doodad
            + m.blocked_path_unavailable
    }
}
