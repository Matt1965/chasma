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
    /// Reserved grid overlay (not rendered yet).
    pub grid: bool,
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
    Combat,
    Health,
    Grid,
}

// --- run_if helpers (REVIEW-A6: skip systems when category disabled) ---

pub fn debug_intent_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Intent)
}

pub fn debug_path_overlay_enabled(settings: &DebugOverlaySettings) -> bool {
    settings.category_enabled(DebugOverlayCategory::Path)
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
debug_overlay_run_if!(run_debug_formation_overlay, debug_formation_overlay_enabled);
debug_overlay_run_if!(run_debug_steering_overlay, debug_steering_overlay_enabled);
debug_overlay_run_if!(run_debug_selection_overlay, debug_selection_overlay_enabled);
debug_overlay_run_if!(run_debug_interaction_overlay, debug_interaction_overlay_enabled);
debug_overlay_run_if!(run_debug_combat_overlay, debug_combat_overlay_enabled);

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
        ] {
            assert!(
                !config.category_enabled(category),
                "{category:?} should be off in production"
            );
        }
    }

    #[test]
    fn default_matches_production() {
        assert_eq!(DebugOverlayConfig::default(), DebugOverlayConfig::production());
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
