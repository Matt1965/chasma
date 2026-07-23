//! Debug overlay toggle settings (ADR-039 U-UI3, ADR-047, REVIEW-A6).

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
    /// Combat range circles, target lines, projectile debug (ADR-061 C8).
    pub combat: bool,
    /// Show health bars for all living units (ADR-062 C9 dev debug).
    pub health: bool,
    /// Reserved grid overlay — walkable navigation cells (NV0).
    pub grid: bool,
    /// Blocked navigation cells colored by passability reason (NV0).
    pub nav_blockers: bool,
    /// Building footprint outlines from occupancy shapes (NV0).
    pub nav_footprints: bool,
    /// Portal / entrance markers (NV0).
    pub nav_entrances: bool,
    /// Construction-reserved occupancy cells (NV0).
    pub nav_reservations: bool,
    /// Static occupancy blocked cells (NV0).
    pub nav_occupancy: bool,
    /// Generated navigation blueprint overlay (NV1.2.5).
    pub nav_blueprint: bool,
    /// Cap per-frame debug draws for moving/selected units.
    pub max_draw_units: u32,
}

/// Back-compat alias used across overlay systems.
pub type DebugOverlaySettings = DebugOverlayConfig;

impl Default for DebugOverlayConfig {
    fn default() -> Self {
        Self::production()
    }
}

impl DebugOverlayConfig {
    /// Production / non-dev defaults: all debug visualization off.
    pub fn production() -> Self {
        Self {
            enabled: false,
            intent: false,
            path: false,
            formation: false,
            steering: false,
            selection: false,
            interaction: false,
            combat: false,
            health: false,
            grid: false,
            nav_blockers: false,
            nav_footprints: false,
            nav_entrances: false,
            nav_reservations: false,
            nav_occupancy: false,
            nav_blueprint: false,
            max_draw_units: 64,
        }
    }

    /// Dev session defaults: master on, categories off until toggled in Dev Mode.
    #[cfg(any(test, feature = "dev"))]
    pub fn development() -> Self {
        Self {
            enabled: true,
            intent: false,
            path: false,
            formation: false,
            steering: false,
            selection: false,
            interaction: false,
            combat: false,
            health: false,
            grid: false,
            nav_blockers: false,
            nav_footprints: false,
            nav_entrances: false,
            nav_reservations: false,
            nav_occupancy: false,
            nav_blueprint: false,
            max_draw_units: 64,
        }
    }

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
            DebugOverlayCategory::Combat => self.combat,
            DebugOverlayCategory::Health => self.health,
            DebugOverlayCategory::Grid => self.grid,
            DebugOverlayCategory::NavBlockers => self.nav_blockers,
            DebugOverlayCategory::NavFootprints => self.nav_footprints,
            DebugOverlayCategory::NavEntrances => self.nav_entrances,
            DebugOverlayCategory::NavReservations => self.nav_reservations,
            DebugOverlayCategory::NavOccupancy => self.nav_occupancy,
            DebugOverlayCategory::NavBlueprint => self.nav_blueprint,
        }
    }

    /// True when any NV0 navigation grid overlay category is enabled.
    pub fn navigation_overlay_active(&self) -> bool {
        self.enabled
            && (self.grid
                || self.nav_blockers
                || self.nav_footprints
                || self.nav_entrances
                || self.nav_reservations
                || self.nav_occupancy)
    }

    /// True when generated blueprint inspection overlay should draw (NV1.2.5).
    pub fn blueprint_overlay_active(&self) -> bool {
        self.enabled && self.nav_blueprint
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
    Combat,
    Health,
    Grid,
    NavBlockers,
    NavFootprints,
    NavEntrances,
    NavReservations,
    NavOccupancy,
    NavBlueprint,
}

// --- run_if helpers (REVIEW-A6: skip systems when category disabled) ---

pub fn debug_intent_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Intent)
}

pub fn debug_path_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Path)
}

pub fn debug_navigation_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.navigation_overlay_active()
}

pub fn debug_formation_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Formation)
}

pub fn debug_steering_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Steering)
}

pub fn debug_selection_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Selection)
}

pub fn debug_interaction_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Interaction)
}

pub fn debug_combat_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Combat)
}

macro_rules! debug_overlay_run_if {
    ($fn_name:ident, $helper:ident) => {
        pub fn $fn_name(settings: Res<DebugOverlaySettings>) -> bool {
            $helper(&settings)
        }
    };
}

debug_overlay_run_if!(run_debug_intent_overlay, debug_intent_overlay_enabled);
debug_overlay_run_if!(run_debug_path_overlay, debug_path_overlay_enabled);
debug_overlay_run_if!(run_debug_navigation_overlay, debug_navigation_overlay_enabled);
debug_overlay_run_if!(run_debug_formation_overlay, debug_formation_overlay_enabled);
debug_overlay_run_if!(run_debug_steering_overlay, debug_steering_overlay_enabled);
debug_overlay_run_if!(run_debug_selection_overlay, debug_selection_overlay_enabled);
debug_overlay_run_if!(
    run_debug_interaction_overlay,
    debug_interaction_overlay_enabled
);
debug_overlay_run_if!(run_debug_combat_overlay, debug_combat_overlay_enabled);

pub fn debug_blueprint_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.blueprint_overlay_active()
}

#[cfg(feature = "dev")]
pub fn debug_blueprint_overlay_or_inspection(
    settings: &DebugOverlaySettings,
    inspection_active: bool,
) -> bool {
    settings.blueprint_overlay_active() || inspection_active
}

#[cfg(feature = "dev")]
pub fn run_debug_blueprint_overlay(
    settings: Res<DebugOverlaySettings>,
    inspection: Res<crate::dev::BlueprintInspectionState>,
) -> bool {
    debug_blueprint_overlay_or_inspection(&settings, inspection.active)
}

#[cfg(not(feature = "dev"))]
pub fn run_debug_blueprint_overlay(settings: Res<DebugOverlaySettings>) -> bool {
    debug_blueprint_overlay_enabled(&settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_defaults_disable_all_debug_categories() {
        let config = DebugOverlayConfig::production();
        assert!(!config.enabled);
        for category in [
            DebugOverlayCategory::Intent,
            DebugOverlayCategory::Path,
            DebugOverlayCategory::Formation,
            DebugOverlayCategory::Steering,
            DebugOverlayCategory::Selection,
            DebugOverlayCategory::Interaction,
            DebugOverlayCategory::Combat,
            DebugOverlayCategory::Health,
            DebugOverlayCategory::Grid,
            DebugOverlayCategory::NavBlockers,
            DebugOverlayCategory::NavFootprints,
            DebugOverlayCategory::NavEntrances,
            DebugOverlayCategory::NavReservations,
            DebugOverlayCategory::NavOccupancy,
            DebugOverlayCategory::NavBlueprint,
        ] {
            assert!(
                !config.category_enabled(category),
                "{category:?} should be off in production"
            );
        }
    }

    #[test]
    fn default_matches_production() {
        assert_eq!(
            DebugOverlayConfig::default(),
            DebugOverlayConfig::production()
        );
    }

    #[cfg(any(test, feature = "dev"))]
    #[test]
    fn development_master_on_categories_off() {
        let config = DebugOverlayConfig::development();
        assert!(config.enabled);
        assert!(!config.path);
        assert!(!config.intent);
        assert!(!config.interaction);
    }

    #[test]
    fn master_switch_disables_categories() {
        let config = DebugOverlayConfig {
            enabled: false,
            path: true,
            intent: true,
            ..DebugOverlayConfig::production()
        };
        assert!(!config.category_enabled(DebugOverlayCategory::Path));
        assert!(!config.category_enabled(DebugOverlayCategory::Intent));
    }

    #[test]
    fn run_if_helpers_match_category_enabled() {
        let config = DebugOverlayConfig {
            enabled: true,
            path: true,
            interaction: false,
            ..DebugOverlayConfig::production()
        };
        assert!(debug_path_overlay_enabled(&config));
        assert!(!debug_interaction_overlay_enabled(&config));
    }
}
