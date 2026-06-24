//! Debug overlay toggle settings (ADR-039 U-UI3, ADR-047).

use bevy::prelude::*;

/// Single source of truth for debug overlay toggles (presentation only).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct DebugOverlayConfig {
    /// Master switch — when false, all overlay systems no-op.
    pub enabled: bool,
    pub intent: bool,
    pub path: bool,
    pub formation: bool,
    pub steering: bool,
    pub selection: bool,
    pub interaction: bool,
    /// Reserved grid overlay (not rendered yet).
    pub grid: bool,
    /// Cap per-frame debug draws for moving/selected units.
    pub max_draw_units: u32,
}

/// Back-compat alias used across overlay systems.
pub type DebugOverlaySettings = DebugOverlayConfig;

impl Default for DebugOverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            intent: true,
            path: true,
            formation: true,
            steering: true,
            selection: true,
            interaction: true,
            grid: false,
            max_draw_units: 64,
        }
    }
}

impl DebugOverlayConfig {
    pub fn category_enabled(&self, category: DebugOverlayCategory) -> bool {
        if !self.enabled {
            return false;
        }
        match category {
            DebugOverlayCategory::Intent => self.intent,
            DebugOverlayCategory::Path => self.path,
            DebugOverlayCategory::Formation => self.formation,
            DebugOverlayCategory::Steering => self.steering,
            DebugOverlayCategory::Selection => self.selection,
            DebugOverlayCategory::Interaction => self.interaction,
            DebugOverlayCategory::Grid => self.grid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOverlayCategory {
    Intent,
    Path,
    Formation,
    Steering,
    Selection,
    Interaction,
    Grid,
}
