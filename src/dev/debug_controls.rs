//! Sync dev debug toggles into U-UI3 overlay settings (ADR-043, ADR-047).

use bevy::prelude::*;

use crate::debug::DebugOverlayConfig;

use super::dev_mode::DevModeState;

/// Push dev config into the live overlay resource each frame.
pub fn sync_dev_debug_controls(
    dev_state: Res<DevModeState>,
    mut overlay: ResMut<DebugOverlayConfig>,
) {
    if !dev_state.enabled {
        return;
    }
    *overlay = dev_state.debug_config;
}

/// Read overlay settings back into dev config (for panel display sync).
pub fn dev_config_from_overlay(settings: &DebugOverlayConfig) -> DebugOverlayConfig {
    *settings
}

/// Legacy alias.
pub fn dev_flags_from_overlay(settings: &DebugOverlayConfig) -> DebugOverlayConfig {
    dev_config_from_overlay(settings)
}

/// Legacy alias.
pub fn apply_dev_debug_flags(flags: DebugOverlayConfig, settings: &mut DebugOverlayConfig) {
    *settings = flags;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_toggles_modify_overlay_state() {
        let mut settings = DebugOverlayConfig::default();
        let flags = DebugOverlayConfig {
            path: false,
            steering: false,
            formation: true,
            selection: false,
            interaction: true,
            intent: false,
            grid: true,
            ..Default::default()
        };
        apply_dev_debug_flags(flags, &mut settings);
        assert!(!settings.path);
        assert!(!settings.steering);
        assert!(settings.formation);
        assert!(!settings.selection);
        assert!(settings.interaction);
        assert!(!settings.intent);
        assert!(settings.grid);
    }
}
