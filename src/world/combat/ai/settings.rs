//! Combat auto-acquisition settings (ADR-062 C9).

use bevy::prelude::*;

/// Tunable combat AI scan parameters — not full tactical AI.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CombatAiSettings {
    pub enabled: bool,
    pub scan_radius_meters: f32,
    pub scan_interval_seconds: f32,
    pub max_units_scanned_per_tick: usize,
    /// When false, player-controllable units never auto-acquire (default).
    pub player_units_auto_acquire: bool,
}

impl Default for CombatAiSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_radius_meters: 24.0,
            scan_interval_seconds: 0.5,
            max_units_scanned_per_tick: 8,
            player_units_auto_acquire: false,
        }
    }
}

impl CombatAiSettings {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            scan_radius_meters: 24.0,
            scan_interval_seconds: 0.5,
            max_units_scanned_per_tick: 8,
            player_units_auto_acquire: false,
        }
    }
}
