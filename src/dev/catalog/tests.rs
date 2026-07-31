//! Catalog Slice 4 tests.

use super::placement_controls::{PlacementUiContext, placement_control_set, placement_ui_context};

use super::state::{next_visible_tab, on_tab_selected, tab_is_visible, visible_tabs};

use super::tabs::tab_label;

use crate::dev::dev_mode::{DefinitionId, DevModeState, DevTab};

use crate::dev::tools::BrushMode;

use crate::dev::window::DevWindowId;

use crate::world::UnitDefinitionId;

#[test]

fn catalog_tabs_are_asset_browsing_only() {
    let tabs = visible_tabs();

    assert_eq!(tabs.len(), 4);

    assert!(tabs.contains(&DevTab::Units));

    assert!(tabs.contains(&DevTab::Items));

    assert!(!tabs.contains(&DevTab::Scenes));

    assert!(!tabs.contains(&DevTab::Debug));

    for tab in tabs {
        assert_ne!(tab_label(*tab), "Placement");
    }
}

#[test]

fn advanced_tabs_are_not_in_catalog() {
    assert!(!tab_is_visible(DevTab::Debug));

    assert!(!tab_is_visible(DevTab::WorldTools));

    assert!(!tab_is_visible(DevTab::Scenes));
}

#[test]

fn contextual_placement_hidden_on_items() {
    let mut state = DevModeState::default();

    state.active_tab = DevTab::Items;

    assert_eq!(
        placement_ui_context(state.active_tab, &state),
        PlacementUiContext::Hidden
    );
}

#[test]

fn unit_selection_shows_unit_controls() {
    let mut state = DevModeState::default();

    state.active_tab = DevTab::Units;

    state.selected_definition = Some(DefinitionId::Unit(UnitDefinitionId::new("wolf")));

    assert_eq!(
        placement_ui_context(state.active_tab, &state),
        PlacementUiContext::Unit
    );
}

#[test]

fn single_brush_hides_count_spacing_radius() {
    let set = placement_control_set(PlacementUiContext::Unit, BrushMode::SingleClick, None, None);

    assert!(set.pattern);

    assert!(!set.count);

    assert!(!set.spacing);

    assert!(!set.radius);
}

#[test]

fn tab_cycle_wraps_catalog_tabs() {
    let next = next_visible_tab(DevTab::Items);

    assert_eq!(next, DevTab::Units);
}

#[test]

fn tab_memory_updates_on_select() {
    let mut session = crate::dev::catalog::CatalogSessionState::default();

    on_tab_selected(&mut session, DevTab::Doodads);

    assert_eq!(session.last_tab, DevTab::Doodads);
}

#[test]

fn windows_launcher_excludes_advanced_entries() {
    for &window in DevWindowId::WINDOWS_LAUNCHER {
        assert!(!DevWindowId::ADVANCED_LAUNCHER.contains(&window));
    }
}
