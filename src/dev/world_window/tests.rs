//! World window tests (Slice 8).

use crate::dev::window::{DevWindowId, DevWindowRegistry};

#[test]
fn world_window_identity() {
    assert_eq!(DevWindowId::World.title(), "World");
    assert!(!DevWindowId::World.default_visible());
}

#[test]
fn open_world_window_shows_in_registry() {
    let mut registry = DevWindowRegistry::default();
    registry.show(DevWindowId::World);
    assert!(registry.is_visible(DevWindowId::World));
    let pos = registry.session(DevWindowId::World).unwrap().position;
    registry.hide(DevWindowId::World);
    registry.show(DevWindowId::World);
    assert_eq!(registry.session(DevWindowId::World).unwrap().position, pos);
}
