//! Catalog UI and placement control tests.

use super::super::dev_mode::DevModeState;
use crate::dev::catalog::components::DevContextualPlacementAction;
use crate::dev::widgets::contains_forbidden_dev_ui_glyph;
use crate::world::relationship::FactionId;
use crate::world::FactionCatalog;

#[test]
fn contextual_placement_hidden_on_items() {
    use super::placement_controls::{PlacementControlSet, placement_control_set, placement_ui_context};
    use crate::dev::dev_mode::{DefinitionId, DevTab};
    use crate::world::{ItemDefinitionId, UnitDefinitionId};

    let mut state = DevModeState::default();
    state.active_tab = DevTab::Items;
    state.selected_definition = Some(DefinitionId::Item(ItemDefinitionId::new("iron_sword")));
    let ctx = placement_ui_context(state.active_tab, &state);
    let controls = placement_control_set(ctx, state.brush.mode, None, None);
    assert_eq!(controls, PlacementControlSet::default());

    state.active_tab = DevTab::Units;
    state.selected_definition = Some(DefinitionId::Unit(UnitDefinitionId::new("wolf")));
    let ctx = placement_ui_context(state.active_tab, &state);
    let controls = placement_control_set(ctx, state.brush.mode, None, None);
    assert!(controls.spawn_controller);
    assert!(controls.spawn_faction);
}

#[test]
fn catalog_placement_glyph_policy() {
    for label in [
        "Pattern",
        "Count +",
        "Count -",
        "Controller: Player",
        "Faction: Player",
        "Rows -",
        "Yaw -",
        "Scale -",
        "Definitions (5) - enabled-only: true - E toggles",
        "Sim: running   tick      0   Space pause   Shift+Space step",
    ] {
        assert!(
            !contains_forbidden_dev_ui_glyph(label),
            "unsupported glyph in `{label}`"
        );
    }
}

#[test]
fn catalog_placement_actions_exclude_deselect() {
    for action in [
        DevContextualPlacementAction::CycleSpawnController,
        DevContextualPlacementAction::CycleBrush,
    ] {
        let name = format!("{action:?}");
        assert!(!name.contains("Deselect"));
    }
}

#[test]
fn spawn_controller_and_faction_labels_are_independent() {
    let mut state = DevModeState::default();
    let factions = FactionCatalog::default();
    assert_eq!(state.spawn_controller_button_label(), "Controller: Player");
    state.spawn_faction_id = FactionId::new("wild");
    assert_eq!(
        state.spawn_faction_button_label(&factions),
        "Faction: Wild"
    );
    state.cycle_spawn_controller();
    assert_eq!(state.spawn_controller_button_label(), "Controller: AI");
    assert_eq!(
        state.spawn_faction_button_label(&factions),
        "Faction: Wild"
    );
    state.select_spawn_faction(FactionId::new("bandits"));
    assert_eq!(state.spawn_controller_button_label(), "Controller: AI");
    assert_eq!(state.spawn_faction_id.as_str(), "bandits");
    assert!(!state.catalog.faction_picker_open);
}

#[test]
fn catalog_row_pool_supports_long_lists() {
    use super::{catalog_row_pool_capacity, visible_row_count};
    use crate::dev::window::CATALOG_MAX_LIST_HEIGHT_PX;

    let capacity = catalog_row_pool_capacity(CATALOG_MAX_LIST_HEIGHT_PX);
    assert!(capacity >= visible_row_count(CATALOG_MAX_LIST_HEIGHT_PX));
}
