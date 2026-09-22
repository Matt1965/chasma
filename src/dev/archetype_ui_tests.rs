use super::dev_mode::{DefinitionId, DevModeState, DevTab};
use crate::world::relationship::SpeciesId;
use crate::world::{
    BuildingArchetypeCatalog, BuildingArchetypeDefinition, BuildingArchetypeId,
    BuildingArchetypeSnapshot, BuildingDefinitionId, UnitArchetypeCatalog, UnitArchetypeDefinition,
    UnitArchetypeId, UnitCatalog, UnitDefinitionId, starter_unit_definitions,
    BuildingLifecycleState, Affiliation,
};

fn unit_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
}

#[test]
fn doodads_and_items_do_not_show_archetype_pane() {
    let mut state = DevModeState::default();
    state.active_tab = DevTab::Doodads;
    assert!(!state.shows_archetype_pane());
    state.active_tab = DevTab::Items;
    assert!(!state.shows_archetype_pane());
}

#[test]
fn units_and_buildings_show_archetype_pane() {
    let mut state = DevModeState::default();
    state.active_tab = DevTab::Units;
    assert!(state.shows_archetype_pane());
    state.active_tab = DevTab::Buildings;
    assert!(state.shows_archetype_pane());
}

#[test]
fn base_selection_change_clears_archetype_selection() {
    let mut state = DevModeState::default();
    state.select_definition(DefinitionId::Unit(UnitDefinitionId::new("bandit")));
    state.selected_unit_archetype = Some(UnitArchetypeId::new("guard"));
    state.select_definition(DefinitionId::Unit(UnitDefinitionId::new("wolf")));
    assert!(state.selected_unit_archetype.is_none());
}

#[test]
fn invalidate_archetype_resets_incompatible_species() {
    let mut state = DevModeState::default();
    let catalog = unit_catalog();
    state.select_definition(DefinitionId::Unit(UnitDefinitionId::new("bandit")));
    state.selected_unit_archetype = Some(UnitArchetypeId::new("guard"));

    let unit_archetypes = UnitArchetypeCatalog::from_definitions(vec![
        UnitArchetypeDefinition {
            id: UnitArchetypeId::new("guard"),
            display_name: "Guard".to_string(),
            applicable_species: vec![SpeciesId::new("human")],
            gold_min: 0,
            gold_max: 0,
            affiliation_override: None,
            equipment: Vec::new(),
            inventory_stacks: Vec::new(),
            dialogue: None,
            enabled: true,
        },
    ])
    .unwrap();
    let building_archetypes = BuildingArchetypeCatalog::default();

    state.select_definition(DefinitionId::Unit(UnitDefinitionId::new("wolf")));
    state.selected_unit_archetype = Some(UnitArchetypeId::new("guard"));
    state.invalidate_archetype_if_inapplicable(&catalog, &unit_archetypes, &building_archetypes);
    assert!(state.selected_unit_archetype.is_none());
}

#[test]
fn building_archetype_invalidated_when_base_changes() {
    let mut state = DevModeState::default();
    let catalog = unit_catalog();
    state.active_tab = DevTab::Buildings;
    state.select_definition(DefinitionId::Building(BuildingDefinitionId::new("hut")));
    state.selected_building_archetype = Some(BuildingArchetypeId::new("planned"));

    let building_archetypes = BuildingArchetypeCatalog::from_definitions(vec![
        BuildingArchetypeDefinition {
            id: BuildingArchetypeId::new("planned"),
            display_name: "Planned".to_string(),
            base_building_id: BuildingDefinitionId::new("hut"),
            snapshot: BuildingArchetypeSnapshot {
                affiliation: Affiliation::Player,
                team_id: None,
                owner_id: None,
                lifecycle_state: BuildingLifecycleState::Planned,
                container_locked: false,
                uniform_scale: 1.0,
                placement_yaw_deg: 0.0,
                extensions: Default::default(),
            },
            capture_metadata: Default::default(),
            members: Vec::new(),
            enabled: true,
        },
    ])
    .unwrap();

    state.select_definition(DefinitionId::Building(BuildingDefinitionId::new("barn")));
    state.selected_building_archetype = Some(BuildingArchetypeId::new("planned"));
    state.invalidate_archetype_if_inapplicable(
        &catalog,
        &UnitArchetypeCatalog::default(),
        &building_archetypes,
    );
    assert!(state.selected_building_archetype.is_none());
}
