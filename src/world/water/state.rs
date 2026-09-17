//! Authoritative gameplay water configuration (simulation / heightfield space).

use bevy::prelude::*;

/// Historical Environment default, in **presentation** Y (not simulation meters).
pub const DEFAULT_PRESENTATION_WATER_LEVEL: f32 = 56.0;

/// Visible water column to enter swimming (tuning).
pub const DEFAULT_SWIM_ENTER_VISIBLE_METERS: f32 = 1.0;

/// Visible water column to exit swimming (tuning, hysteresis).
pub const DEFAULT_SWIM_EXIT_VISIBLE_METERS: f32 = 0.6;

/// Visible feet-below-surface offset while swimming (tuning; feet-origin meshes).
pub const DEFAULT_SWIM_ORIGIN_OFFSET_VISIBLE_METERS: f32 = -0.9;

/// Water travel relative to catalog `move_speed_mps` (tuning).
pub const DEFAULT_SWIM_SPEED_MULTIPLIER: f32 = 0.5;

/// Convert a presentation/render Y into simulation meters.
pub fn presentation_to_sim(presentation_y: f32, vertical_scale: f32) -> f32 {
    let scale = if vertical_scale.is_finite() && vertical_scale.abs() > 1e-8 {
        vertical_scale
    } else {
        1.0
    };
    presentation_y / scale
}

/// Convert simulation meters into presentation/render Y.
pub fn sim_to_presentation(sim_y: f32, vertical_scale: f32) -> f32 {
    let scale = if vertical_scale.is_finite() && vertical_scale.abs() > 1e-8 {
        vertical_scale
    } else {
        1.0
    };
    sim_y * scale
}

/// Gameplay-owned water surface and swimming tuning.
///
/// `surface_y_sim` and depth thresholds are heightfield meters. Presentation
/// (`WaterSettings.water_level`) is derived via [`sim_to_presentation`].
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct WorldWaterState {
    pub enabled: bool,
    pub surface_y_sim: f32,
    pub enter_depth_sim: f32,
    pub exit_depth_sim: f32,
    pub speed_multiplier: f32,
    pub origin_offset_sim: f32,
    /// When set, first scale-aware sync converts this presentation Y into sim.
    presentation_seed_y: Option<f32>,
}

impl Default for WorldWaterState {
    fn default() -> Self {
        Self {
            // Tests construct bare `WorldData`; keep dry-land behavior unless enabled.
            enabled: false,
            surface_y_sim: 0.0,
            enter_depth_sim: DEFAULT_SWIM_ENTER_VISIBLE_METERS,
            exit_depth_sim: DEFAULT_SWIM_EXIT_VISIBLE_METERS,
            speed_multiplier: DEFAULT_SWIM_SPEED_MULTIPLIER,
            origin_offset_sim: DEFAULT_SWIM_ORIGIN_OFFSET_VISIBLE_METERS,
            presentation_seed_y: Some(DEFAULT_PRESENTATION_WATER_LEVEL),
        }
    }
}

impl WorldWaterState {
    /// Enable water at explicit simulation values (tests / tooling).
    pub fn configure_for_test(
        &mut self,
        enabled: bool,
        surface_y_sim: f32,
        enter_depth_sim: f32,
        exit_depth_sim: f32,
    ) {
        self.enabled = enabled;
        self.surface_y_sim = surface_y_sim;
        self.enter_depth_sim = enter_depth_sim.max(self.exit_depth_sim.min(enter_depth_sim));
        self.exit_depth_sim = exit_depth_sim.min(self.enter_depth_sim);
        self.presentation_seed_y = None;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Write sea level from a Dev/presentation slider value.
    pub fn set_surface_from_presentation(&mut self, presentation_y: f32, vertical_scale: f32) {
        self.surface_y_sim = presentation_to_sim(presentation_y, vertical_scale);
        self.presentation_seed_y = None;
    }

    pub fn presentation_surface_y(&self, vertical_scale: f32) -> f32 {
        sim_to_presentation(self.surface_y_sim, vertical_scale)
    }

    /// Apply vertical scale to the presentation seed and visible-depth tuning once.
    pub fn apply_presentation_scale(&mut self, vertical_scale: f32) {
        let Some(seed) = self.presentation_seed_y else {
            return;
        };
        self.surface_y_sim = presentation_to_sim(seed, vertical_scale);
        self.enter_depth_sim =
            presentation_to_sim(DEFAULT_SWIM_ENTER_VISIBLE_METERS, vertical_scale);
        self.exit_depth_sim = presentation_to_sim(DEFAULT_SWIM_EXIT_VISIBLE_METERS, vertical_scale);
        self.origin_offset_sim =
            presentation_to_sim(DEFAULT_SWIM_ORIGIN_OFFSET_VISIBLE_METERS, vertical_scale);
        if self.exit_depth_sim > self.enter_depth_sim {
            self.exit_depth_sim = self.enter_depth_sim;
        }
        self.presentation_seed_y = None;
    }

    pub fn has_presentation_seed(&self) -> bool {
        self.presentation_seed_y.is_some()
    }
}
