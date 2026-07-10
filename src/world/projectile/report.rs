//! Projectile combat trace events (ADR-060 C7).

use super::id::ProjectileId;
use crate::world::combat::ProjectileImpactRejection;
use crate::world::unit::UnitId;
use crate::world::weapon::WeaponDefinitionId;

/// Projectile simulation trace events.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectileEvent {
    Spawned,
    Hit,
    Expired,
    ImpactRejected {
        reason: ProjectileImpactRejection,
    },
    DamageApplied {
        damage: f32,
        target_hp_before: u32,
        target_hp_after: u32,
    },
}

/// One projectile trace row.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileTrace {
    pub projectile_id: ProjectileId,
    pub source_unit_id: UnitId,
    pub target_unit_id: UnitId,
    pub weapon_id: WeaponDefinitionId,
    pub event: ProjectileEvent,
}

/// Aggregated projectile tick report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectileReport {
    pub traces: Vec<ProjectileTrace>,
}

impl ProjectileReport {
    pub fn push(&mut self, trace: ProjectileTrace) {
        self.traces.push(trace);
    }

    pub fn has_event(&self, event: &ProjectileEvent) -> bool {
        self.traces.iter().any(|trace| &trace.event == event)
    }

    /// Projectile ids spawned during the current strike phase (same tick).
    pub fn spawned_projectile_ids(&self) -> Vec<ProjectileId> {
        self.traces
            .iter()
            .filter(|trace| trace.event == ProjectileEvent::Spawned)
            .map(|trace| trace.projectile_id)
            .collect()
    }
}
