//! Per-unit weapon attack phase state (ADR-058 C5).

use bevy::prelude::*;

use super::id::UnitId;

/// Phase within one weapon attack cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum AttackPhase {
    #[default]
    Windup,
    Strike,
    Recovery,
    Cooldown,
}

/// Active attack cycle for one attacker/target pair.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct AttackCycle {
    pub target: UnitId,
    pub phase: AttackPhase,
    pub phase_remaining_seconds: f32,
    pub struck_this_cycle: bool,
}

impl AttackCycle {
    pub fn start_windup(target: UnitId, windup_seconds: f32) -> Self {
        Self {
            target,
            phase: AttackPhase::Windup,
            phase_remaining_seconds: windup_seconds,
            struck_this_cycle: false,
        }
    }

    pub fn begin_recovery(&mut self, recovery_seconds: f32) {
        self.phase = AttackPhase::Recovery;
        self.phase_remaining_seconds = recovery_seconds;
    }

    pub fn begin_cooldown(&mut self, cooldown_seconds: f32) {
        self.phase = AttackPhase::Cooldown;
        self.phase_remaining_seconds = cooldown_seconds;
    }

    pub fn restart_windup(&mut self, windup_seconds: f32) {
        self.phase = AttackPhase::Windup;
        self.phase_remaining_seconds = windup_seconds;
        self.struck_this_cycle = false;
    }
}
