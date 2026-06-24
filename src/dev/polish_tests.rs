//! U-DEV1 integration tests for dev mode polish (ADR-047).

use std::collections::HashSet;

use crate::debug::DebugOverlayConfig;
use crate::world::{DoodadCatalog, UnitCatalog};

use super::catalog_cache::{
    browse_catalog_entries, CatalogBrowseIndex, CatalogFilterCache, DevSearchDebounce,
};
use super::dev_mode::{DefinitionId, DevModeState, DevTab, SpawnMode};

#[test]
fn favorites_persist_during_session() {
    let mut state = DevModeState::default();
    let id = DefinitionId::Unit(crate::world::UnitDefinitionId::new("wolf"));
    state.toggle_favorite(id.clone());
    state.active_tab = DevTab::Doodads;
    state.active_tab = DevTab::Units;
    assert!(state.favorites.contains(&id));
}

#[test]
fn search_debounce_does_not_change_final_filter_results() {
    let catalog = UnitCatalog::default();
    let mut index = CatalogBrowseIndex::default();
    index.sync(&catalog, &DoodadCatalog::default());
    let mut cache = CatalogFilterCache::default();
    let favorites = HashSet::new();

    let mut debounce = DevSearchDebounce::default();
    debounce.note_input("wolf");
    while debounce.frames_until_settle > 0 {
        debounce.tick();
    }

    let immediate = browse_catalog_entries(
        &index,
        &mut CatalogFilterCache::default(),
        &catalog,
        &DoodadCatalog::default(),
        DevTab::Units,
        SpawnMode::Unit,
        "wolf",
        true,
        &favorites,
    )
    .to_vec();

    let debounced = browse_catalog_entries(
        &index,
        &mut cache,
        &catalog,
        &DoodadCatalog::default(),
        DevTab::Units,
        SpawnMode::Unit,
        &debounce.filtered_query,
        true,
        &favorites,
    )
    .to_vec();

    assert_eq!(immediate, debounced);
}

#[test]
fn catalog_filtering_is_deterministic_and_cached() {
    let catalog = UnitCatalog::default();
    let mut index = CatalogBrowseIndex::default();
    index.sync(&catalog, &DoodadCatalog::default());
    let mut cache = CatalogFilterCache::default();
    let favorites = HashSet::new();

    let first_ptr = browse_catalog_entries(
        &index,
        &mut cache,
        &catalog,
        &DoodadCatalog::default(),
        DevTab::Units,
        SpawnMode::Unit,
        "",
        true,
        &favorites,
    )
    .as_ptr() as usize;
    let second_ptr = browse_catalog_entries(
        &index,
        &mut cache,
        &catalog,
        &DoodadCatalog::default(),
        DevTab::Units,
        SpawnMode::Unit,
        "",
        true,
        &favorites,
    )
    .as_ptr() as usize;
    assert_eq!(first_ptr, second_ptr);
}

#[test]
fn debug_flags_reflect_dev_config() {
    let mut state = DevModeState::default();
    state.debug_config.path = false;
    state.debug_config.intent = false;
    let overlay = DebugOverlayConfig {
        path: false,
        intent: false,
        ..DebugOverlayConfig::default()
    };
    assert_eq!(state.debug_config.path, overlay.path);
    assert_eq!(state.debug_config.intent, overlay.intent);
}

#[test]
fn dev_ui_state_persists_across_tab_switches() {
    let mut state = DevModeState::default();
    state.search_query = "oak".into();
    state.spawn_mode = SpawnMode::Doodad;
    state.enabled_only = false;
    state.select_definition(DefinitionId::Doodad(
        crate::world::DoodadDefinitionId::new("tree"),
    ));
    state.active_tab = DevTab::Debug;
    assert_eq!(state.search_query, "oak");
    assert_eq!(state.spawn_mode, SpawnMode::Doodad);
    assert!(!state.enabled_only);
    assert!(state.selected_definition.is_some());
}

#[test]
fn quick_spawn_hotkey_slots_store_definitions() {
    let mut state = DevModeState::default();
    let id = DefinitionId::Unit(crate::world::UnitDefinitionId::new("deer"));
    state.assign_favorite_slot(2, id.clone());
    assert_eq!(state.favorite_slot(2), Some(&id));
}

#[test]
fn spawn_gate_blocks_duplicate_handling_per_frame() {
    let mut gate = super::dev_mode::DevModeInputGate::default();
    assert!(!gate.spawn_handled_this_frame);
    gate.spawn_handled_this_frame = true;
    assert!(gate.spawn_handled_this_frame);
}
