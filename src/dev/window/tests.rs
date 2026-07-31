//! Additional dev-window integration tests.

use bevy::prelude::*;

use super::DevWindowId;

use super::math::default_catalog_position;

use super::state::DevWindowRegistry;

#[test]

fn registry_default_position_is_top_right_bias() {
    let registry = DevWindowRegistry::default();

    let pos = registry.session(DevWindowId::Catalog).unwrap().position;

    let expected = default_catalog_position(registry.viewport, 368.0);

    assert!((pos.x - expected.x).abs() < 0.01);

    assert!((pos.y - expected.y).abs() < 0.01);
}

#[test]

fn reopen_after_hide_restores_visibility_without_resetting_position() {
    let mut registry = DevWindowRegistry::default();

    let pos = registry.session(DevWindowId::Catalog).unwrap().position;

    registry.hide(DevWindowId::Catalog);

    assert!(!registry.is_visible(DevWindowId::Catalog));

    registry.show(DevWindowId::Catalog);

    assert!(registry.is_visible(DevWindowId::Catalog));

    assert_eq!(
        registry.session(DevWindowId::Catalog).unwrap().position,
        pos
    );
}

#[test]

fn f12_off_cancels_drag_state() {
    let mut registry = DevWindowRegistry::default();

    registry.begin_drag(DevWindowId::Catalog, Vec2::new(5.0, 5.0));

    registry.cancel_drag();

    assert!(registry.drag.is_none());
}

#[test]

fn windows_launcher_order() {
    assert_eq!(
        DevWindowId::WINDOWS_LAUNCHER,
        &[
            DevWindowId::Save,
            DevWindowId::Catalog,
            DevWindowId::SelectedObject,
        ]
    );
}

#[test]

fn advanced_launcher_order_and_labels() {
    assert_eq!(
        DevWindowId::ADVANCED_LAUNCHER,
        &[
            DevWindowId::Debug,
            DevWindowId::World,
            DevWindowId::Fields,
            DevWindowId::NavigationEditor,
        ]
    );

    assert_eq!(
        DevWindowId::NavigationEditor.launcher_label(),
        "Navigation Editor"
    );
}

#[test]
fn launcher_toggle_hides_and_shows_window() {
    let mut registry = DevWindowRegistry::default();
    assert!(!registry.is_visible(DevWindowId::Debug));
    registry.toggle(DevWindowId::Debug);
    assert!(registry.is_visible(DevWindowId::Debug));
    let pos = registry.session(DevWindowId::Debug).unwrap().position;
    registry.toggle(DevWindowId::Debug);
    assert!(!registry.is_visible(DevWindowId::Debug));
    registry.toggle(DevWindowId::Debug);
    assert!(registry.is_visible(DevWindowId::Debug));
    assert_eq!(registry.session(DevWindowId::Debug).unwrap().position, pos);
}

#[test]

fn advanced_launcher_defaults_collapsed() {
    let registry = DevWindowRegistry::default();

    assert!(registry.launcher_expanded);

    assert!(!registry.advanced_launcher_expanded);
}

#[test]

fn focus_reuses_open_window() {
    let mut registry = DevWindowRegistry::default();

    registry.show(DevWindowId::Debug);

    registry.show(DevWindowId::World);

    registry.show(DevWindowId::Debug);

    assert!(registry.is_visible(DevWindowId::Debug));

    assert_eq!(registry.focus_stack.last(), Some(&DevWindowId::Debug));
}
