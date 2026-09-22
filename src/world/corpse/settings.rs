use bevy::prelude::*;

/// Default corpse interaction radius (matches interaction query default).
const DEFAULT_CORPSE_INTERACTION_RADIUS_METERS: f32 = 2.5;

/// Global corpse lifetime defaults (ADR-089 I3).
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Resource)]
pub struct CorpseSettings {
    /// Default authoritative lifetime when a unit definition omits an override.
    pub default_lifetime_ticks: u64,
    /// Horizontal interaction hit radius in meters (ADR-090 I4 / ADR-042).
    pub interaction_radius_meters: f32,
}

impl Default for CorpseSettings {
    fn default() -> Self {
        Self {
            // 5 minutes at 30 Hz simulation tick.
            default_lifetime_ticks: 9_000,
            interaction_radius_meters: DEFAULT_CORPSE_INTERACTION_RADIUS_METERS,
        }
    }
}

impl CorpseSettings {
    pub fn interaction_radius_squared_cm(&self) -> i64 {
        let radius_cm = (self.interaction_radius_meters * 100.0).round() as i64;
        radius_cm.saturating_mul(radius_cm)
    }
}

pub const DEFAULT_CORPSE_LIFETIME_TICKS: u64 = 9_000;
