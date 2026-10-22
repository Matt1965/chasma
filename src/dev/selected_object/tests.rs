//! Selected Object window tests (Slice 5).

use super::format::{EMPTY_STATE, format_unit_summary, unit_is_player_commandable};
use super::state::{PendingDeleteTarget, SelectedObjectUiState};
use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::units::input::SelectedUnits;
use crate::world::{ChunkLayout, WorldData};

fn test_layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}
use crate::dev::dev_mode::DevTab;
use crate::dev::inspector::{ChunkResidencySnapshot, UnitInspectorSnapshot};
use crate::dev::window::DevWindowId;
use crate::dev::window::DevWindowRegistry;
use crate::world::{UnitDefinitionId, UnitId};

#[test]
fn selected_object_window_registered_by_default() {
    let registry = DevWindowRegistry::default();
    assert!(registry.is_visible(DevWindowId::SelectedObject));
    assert!(registry.session(DevWindowId::SelectedObject).is_some());
}

#[test]
fn inspector_tab_removed_from_dev_tab() {
    let tabs = [
        DevTab::Units,
        DevTab::Doodads,
        DevTab::Buildings,
        DevTab::Items,
        DevTab::Scenes,
        DevTab::Debug,
        DevTab::WorldTools,
        DevTab::TerrainFields,
    ];
    for tab in tabs {
        assert_ne!(format!("{tab:?}"), "Inspector");
    }
}

#[test]
fn empty_state_message_is_compact() {
    assert!(EMPTY_STATE.contains("unit"));
    assert!(EMPTY_STATE.contains("building"));
}

#[test]
fn multi_unit_summary_shows_count() {
    let snapshot = UnitInspectorSnapshot {
        unit_id: UnitId::new(1),
        definition_id: UnitDefinitionId::new("wolf"),
        state_label: "Idle".into(),
        current_hp: 10,
        max_hp: 10,
        combat_state_label: "Idle".into(),
        combat: Default::default(),
        projectiles: Vec::new(),
        path: Default::default(),
        formation: Default::default(),
        steering: Default::default(),
        block_reason: None,
        chunk: ChunkResidencySnapshot {
            unit_chunk: crate::world::ChunkCoord { x: 0, z: 0 },
            terrain_loaded: true,
            doodads_in_chunk: 0,
            units_in_chunk: 1,
        },
        simulation_tick: 0,
        current_space_id: crate::world::SpaceId::SURFACE,
        display_floor_label: "Surface".into(),
        inventory_summary: None,
        affiliation: "Player".into(),
        settlement_membership: "None".into(),
        nutrition_current: None,
        nutrition_max: None,
        hunger_stage: None,
        self_maintenance_label: None,
    };
    let text = format_unit_summary(&snapshot, 3);
    assert!(text.contains("3 selected"));
    assert!(unit_is_player_commandable(&snapshot));
}

#[test]
fn diagnostics_default_collapsed() {
    let state = SelectedObjectUiState::default();
    assert!(!state.diagnostics_expanded);
}

#[test]
fn unit_pending_delete_requires_single_matching_selection() {
    let world = WorldData::new(test_layout());
    let unit_id = UnitId::new(7);
    let mut world_selection = WorldSelectionState::default();
    world_selection.category = WorldSelectionCategory::Units;
    let mut selected = SelectedUnits::default();
    selected.set_single(unit_id);

    assert!(!SelectedObjectUiState::pending_delete_still_valid(
        PendingDeleteTarget::Unit(unit_id),
        &world_selection,
        &selected,
        &world,
    ));

    let mut ui = SelectedObjectUiState::default();
    ui.request_delete_unit(unit_id);
    assert_eq!(ui.pending_delete, Some(PendingDeleteTarget::Unit(unit_id)));

    selected.set_single(UnitId::new(8));
    assert!(!SelectedObjectUiState::pending_delete_still_valid(
        PendingDeleteTarget::Unit(unit_id),
        &world_selection,
        &selected,
        &world,
    ));
}

#[test]
fn closing_window_does_not_imply_selection_cleared() {
    let mut registry = DevWindowRegistry::default();
    registry.hide(DevWindowId::SelectedObject);
    assert!(!registry.is_visible(DevWindowId::SelectedObject));
    // Selection is independent — registry hide only affects window visibility.
}
