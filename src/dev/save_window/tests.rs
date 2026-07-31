//! Save window tests.

use crate::dev::window::{DevWindowId, DevWindowRegistry};

#[test]
fn save_window_has_stable_id() {
    assert_eq!(DevWindowId::Save.title(), "Save");
    assert_eq!(DevWindowId::Save.launcher_label(), "Save");
}

#[test]
fn save_defaults_closed() {
    let registry = DevWindowRegistry::default();
    assert!(!registry.is_visible(DevWindowId::Save));
}

#[test]
fn save_is_first_windows_launcher_entry() {
    assert_eq!(DevWindowId::WINDOWS_LAUNCHER[0], DevWindowId::Save);
    assert_eq!(DevWindowId::WINDOWS_LAUNCHER[1], DevWindowId::Catalog);
}

#[test]
fn open_save_focuses_existing_window() {
    let mut registry = DevWindowRegistry::default();
    let pos = registry.session(DevWindowId::Save).unwrap().position;
    registry.show(DevWindowId::Save);
    registry.show(DevWindowId::Save);
    assert!(registry.is_visible(DevWindowId::Save));
    assert_eq!(registry.session(DevWindowId::Save).unwrap().position, pos);
}
