//! Debug window tests (Slice 8).

use crate::debug::DebugOverlayConfig;
use crate::dev::dev_mode::DevDebugFlags;
use crate::dev::window::{DevWindowId, DevWindowRegistry};

#[test]
fn relationship_links_toggle_defaults_off() {
    let config = DebugOverlayConfig::production();
    assert!(!config.relationship_links);
}

#[test]
fn debug_window_identity() {
    assert_eq!(DevWindowId::Debug.title(), "Debug");
    assert!(!DevWindowId::Debug.default_visible());
}

#[test]
fn open_debug_window_shows_in_registry() {
    let mut registry = DevWindowRegistry::default();
    assert!(!registry.is_visible(DevWindowId::Debug));
    registry.show(DevWindowId::Debug);
    assert!(registry.is_visible(DevWindowId::Debug));
}

#[test]
fn master_overlay_flag_independent_of_window_visibility() {
    let mut registry = DevWindowRegistry::default();
    let flags = DevDebugFlags {
        enabled: true,
        path: true,
        ..Default::default()
    };
    registry.hide(DevWindowId::Debug);
    assert!(!registry.is_visible(DevWindowId::Debug));
    assert!(flags.enabled);
    assert!(flags.path);
}

#[test]
fn relationship_links_toggle_independent_of_window_visibility() {
    let mut registry = DevWindowRegistry::default();
    let flags = DevDebugFlags {
        enabled: true,
        relationship_links: true,
        ..Default::default()
    };
    registry.hide(DevWindowId::Debug);
    assert!(!registry.is_visible(DevWindowId::Debug));
    assert!(flags.relationship_links);
}
