//! Fields window tests (Slice 8).

use crate::dev::window::{DevWindowId, DevWindowRegistry};

#[test]
fn fields_window_identity() {
    assert_eq!(DevWindowId::Fields.title(), "Fields");
    assert!(!DevWindowId::Fields.default_visible());
}

#[test]
fn fields_window_default_position_offset_from_catalog() {
    let registry = DevWindowRegistry::default();
    let fields_pos = registry.session(DevWindowId::Fields).unwrap().position;
    let catalog_pos = registry.session(DevWindowId::Catalog).unwrap().position;
    assert_ne!(fields_pos, catalog_pos);
}
