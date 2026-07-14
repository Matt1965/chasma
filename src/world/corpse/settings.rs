use bevy::prelude::*;

/// Global corpse lifetime defaults (ADR-089 I3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource)]
pub struct CorpseSettings {
    /// Default authoritative lifetime when a unit definition omits an override.
    pub default_lifetime_ticks: u64,
}

impl Default for CorpseSettings {
    fn default() -> Self {
        Self {
            // 5 minutes at 30 Hz simulation tick.
            default_lifetime_ticks: 9_000,
        }
    }
}

pub const DEFAULT_CORPSE_LIFETIME_TICKS: u64 = 9_000;
