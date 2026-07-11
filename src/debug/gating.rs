//! REVIEW-A6 production gating and read-only overlay tests.

#[cfg(test)]
mod tests {
    use crate::debug::trace::{CommandTraceBuffer, IntentDispatchHistory};
    use crate::debug::{DebugOverlayCategory, DebugOverlayConfig};

    #[test]
    fn command_trace_resources_exist_without_visualization_enabled() {
        let config = DebugOverlayConfig::production();
        assert!(!config.category_enabled(DebugOverlayCategory::Intent));
        let _trace = CommandTraceBuffer::default();
        let _history = IntentDispatchHistory::default();
    }

    #[test]
    fn production_config_has_no_visible_debug_overlays_by_default() {
        let config = DebugOverlayConfig::production();
        assert!(!config.enabled);
        assert!(!debug_category_visible(&config, DebugOverlayCategory::Path));
        assert!(!debug_category_visible(
            &config,
            DebugOverlayCategory::Interaction
        ));
        assert!(!debug_category_visible(
            &config,
            DebugOverlayCategory::Intent
        ));
        assert!(!debug_category_visible(
            &config,
            DebugOverlayCategory::Steering
        ));
        assert!(!debug_category_visible(
            &config,
            DebugOverlayCategory::Formation
        ));
        assert!(!debug_category_visible(
            &config,
            DebugOverlayCategory::Selection
        ));
        assert!(!debug_category_visible(
            &config,
            DebugOverlayCategory::Combat
        ));
        assert!(!debug_category_visible(&config, DebugOverlayCategory::Grid));
    }

    #[test]
    fn disabled_path_overlay_run_if_returns_false_in_production() {
        let config = DebugOverlayConfig::production();
        assert!(!crate::debug::debug_path_overlay_enabled(&config));
    }

    #[test]
    fn disabled_interaction_capture_run_if_returns_false_in_production() {
        let config = DebugOverlayConfig::production();
        assert!(!crate::debug::debug_interaction_overlay_enabled(&config));
    }

    fn debug_category_visible(config: &DebugOverlayConfig, category: DebugOverlayCategory) -> bool {
        config.category_enabled(category)
    }
}
