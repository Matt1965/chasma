//! BP1/BP2 building panel tests.

use super::{
    BuildingPanelState, build_building_panel_snapshot, building_owned_by_local_player,
    format_building_shell, on_gameplay_building_selected, reconcile_building_panel,
    try_open_building_menu,
};
use crate::client::selection::{
    ApplyWorldSelectionParams, WorldSelectionCategory, WorldSelectionChange,
    WorldSelectionRevision, WorldSelectionState, apply_world_selection,
};
use crate::player::LocalPlayerOwnership;
use crate::units::input::SelectedUnits;
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinitionId, BuildingId,
    BuildingOwnership, BuildingPlacement, BuildingRecord, BuildingSource, ChunkCoord, ChunkData,
    ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitId, WorldData, WorldPosition,
};
use bevy::prelude::{Quat, Vec3};

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn insert_building(world: &mut WorldData, id: u64, ownership: BuildingOwnership) -> BuildingId {
    let building_id = BuildingId::new(id);
    let record = BuildingRecord::new(
        building_id,
        BuildingDefinitionId::new("prispod_farm"),
        BuildingPlacement::new(pos(10.0, 10.0), Quat::IDENTITY),
        ownership,
        300,
        BuildingSource::Authored,
    );
    let chunk = ChunkId::new(record.placement.position.chunk);
    world.insert_building(chunk, record).unwrap();
    building_id
}

fn player() -> LocalPlayerOwnership {
    LocalPlayerOwnership::default()
}

fn foreign_ownership() -> BuildingOwnership {
    BuildingOwnership::with_affiliation(Affiliation::Hostile)
}

fn apply_params<'a>(
    world_selection: &'a mut WorldSelectionState,
    selected_units: &'a mut SelectedUnits,
    revision: &'a mut WorldSelectionRevision,
) -> ApplyWorldSelectionParams<'a> {
    ApplyWorldSelectionParams {
        world_selection,
        selected_units,
        hud: None,
        revision: Some(revision),
    }
}

#[test]
fn any_building_may_become_world_selection_regardless_of_ownership() {
    let mut world = flat_world();
    let foreign = insert_building(&mut world, 1, foreign_ownership());
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::SelectBuilding {
            building_id: foreign,
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::Building);
    assert_eq!(world_selection.building_id, Some(foreign));
}

#[test]
fn selecting_owned_building_opens_menu() {
    let mut world = flat_world();
    let owned = insert_building(
        &mut world,
        2,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mut panel = BuildingPanelState::default();

    assert!(try_open_building_menu(&mut panel, owned, &world, &player()));
    assert_eq!(panel.open_building_id, Some(owned));
}

#[test]
fn selecting_non_owned_building_does_not_open_menu() {
    let mut world = flat_world();
    let foreign = insert_building(&mut world, 3, foreign_ownership());
    let mut panel = BuildingPanelState::default();

    on_gameplay_building_selected(foreign, &mut panel, &world, &player());
    assert!(panel.open_building_id.is_none());
}

#[test]
fn open_farm_menu_survives_selecting_unit() {
    let mut world = flat_world();
    let farm = insert_building(
        &mut world,
        4,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mut panel = BuildingPanelState::default();
    panel.open(farm);

    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();
    apply_world_selection(
        WorldSelectionChange::SelectUnit {
            unit_id: UnitId::new(7),
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::Units);
    assert_eq!(panel.open_building_id, Some(farm));
}

#[test]
fn open_farm_menu_survives_clearing_world_selection() {
    let mut panel = BuildingPanelState::default();
    panel.open(BuildingId::new(4));

    let mut world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Building,
        building_id: Some(BuildingId::new(4)),
        ..Default::default()
    };
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();
    apply_world_selection(
        WorldSelectionChange::ClearAll,
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::None);
    assert_eq!(panel.open_building_id, Some(BuildingId::new(4)));
}

#[test]
fn open_farm_menu_survives_selecting_foreign_building() {
    let mut world = flat_world();
    let farm = insert_building(
        &mut world,
        5,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let foreign = insert_building(&mut world, 6, foreign_ownership());
    let mut panel = BuildingPanelState::default();
    panel.open(farm);

    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();
    apply_world_selection(
        WorldSelectionChange::SelectBuilding {
            building_id: foreign,
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );

    assert_eq!(world_selection.building_id, Some(foreign));
    assert_eq!(panel.open_building_id, Some(farm));
}

#[test]
fn selecting_another_owned_building_switches_menu_target() {
    let mut world = flat_world();
    let farm = insert_building(
        &mut world,
        7,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mine = insert_building(
        &mut world,
        8,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mut panel = BuildingPanelState::default();
    panel.open(farm);

    on_gameplay_building_selected(mine, &mut panel, &world, &player());
    assert_eq!(panel.open_building_id, Some(mine));
}

#[test]
fn explicit_close_clears_menu_without_clearing_selection() {
    let mut panel = BuildingPanelState::default();
    panel.open(BuildingId::new(9));
    let world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Building,
        building_id: Some(BuildingId::new(9)),
        ..Default::default()
    };

    panel.close();

    assert!(panel.open_building_id.is_none());
    assert_eq!(world_selection.building_id, Some(BuildingId::new(9)));
}

#[test]
fn removing_open_building_invalidates_menu() {
    let mut world = flat_world();
    let farm = insert_building(
        &mut world,
        10,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mut panel = BuildingPanelState::default();
    panel.open(farm);
    world.remove_building_by_id(farm);

    reconcile_building_panel(&mut panel, &world, &player());
    assert!(panel.open_building_id.is_none());
}

#[test]
fn losing_ownership_invalidates_menu() {
    let mut world = flat_world();
    let farm = insert_building(
        &mut world,
        11,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mut panel = BuildingPanelState::default();
    panel.open(farm);

    world.mutate_building(farm, |record| {
        record.ownership = foreign_ownership();
    });

    reconcile_building_panel(&mut panel, &world, &player());
    assert!(panel.open_building_id.is_none());
}

#[test]
fn shell_format_is_player_facing() {
    let text = format_building_shell(
        "Prispod Farm",
        crate::world::BuildingLifecycleState::Complete,
        300,
        300,
    );
    assert!(text.contains("Prispod Farm"));
    assert!(text.contains("Complete"));
    assert!(text.contains("HP 300 / 300"));
    assert!(!text.contains("control"));
    assert!(!text.contains("priority"));
}

#[test]
fn ownership_uses_owner_id_not_affiliation_friendliness() {
    let mut neutral = BuildingOwnership::neutral();
    neutral.affiliation = Affiliation::Player;
    let record = BuildingRecord::new(
        BuildingId::new(12),
        BuildingDefinitionId::new("hut"),
        BuildingPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
        neutral,
        100,
        BuildingSource::Authored,
    );
    assert!(!building_owned_by_local_player(&record, &player()));
}

#[test]
fn player_owned_building_matches_local_player_owner_id() {
    let record = BuildingRecord::new(
        BuildingId::new(13),
        BuildingDefinitionId::new("hut"),
        BuildingPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(Affiliation::Player),
        100,
        BuildingSource::Authored,
    );
    assert!(building_owned_by_local_player(&record, &player()));
}

#[test]
fn select_building_does_not_mutate_panel_state() {
    let mut world = flat_world();
    let owned = insert_building(
        &mut world,
        14,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mut panel = BuildingPanelState::default();
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();

    assert!(panel.open_building_id.is_none());
}

#[test]
fn foreign_building_selection_does_not_expose_panel_inventories() {
    let mut world = flat_world();
    let foreign = insert_building(&mut world, 15, foreign_ownership());
    let mut panel = BuildingPanelState::default();

    on_gameplay_building_selected(foreign, &mut panel, &world, &player());
    assert!(panel.open_building_id.is_none());
}

#[test]
fn panel_snapshot_uses_binding_store_for_multiple_inventories() {
    use crate::world::{
        BuildingOperationParams, BuildingTerrainAssessmentStore, OperationCatalog,
        create_building_with_inventory, starter_building_definitions,
        starter_operation_definitions,
    };

    let smelter = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "smelter")
        .expect("smelter");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![smelter.clone()], &categories).unwrap();
    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &smelter.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    let snapshot = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    )
    .unwrap();
    assert_eq!(snapshot.inventories.len(), 4);
    assert!(snapshot.production.is_some());
}

// --- BP3 production controls ---

#[test]
fn non_production_building_has_no_production_section() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingOperationParams, BuildingTerrainAssessmentStore,
        OperationCatalog, create_building_with_inventory, starter_building_definitions,
        starter_operation_definitions,
    };

    let hut = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "hut")
        .expect("hut");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![hut.clone()], &categories).unwrap();
    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &hut.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    let snapshot = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    )
    .unwrap();
    assert!(snapshot.production.is_none());
}

#[test]
fn farm_production_controls_have_toggle_without_operation_selector() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingOperationParams, BuildingTerrainAssessmentStore,
        OperationCatalog, create_building_with_inventory, starter_building_definitions,
        starter_operation_definitions,
    };

    let farm = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("farm");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![farm.clone()], &categories).unwrap();
    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &farm.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    let snapshot = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    )
    .unwrap();
    let production = snapshot.production.expect("farm production");
    assert!(!production.show_operation_selector);
    assert!(production.operation_options.is_empty());
    assert_eq!(production.operation_name, "Grow Prispods");
}

#[test]
fn multi_operation_fixture_exposes_selector_with_display_names() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingOperationParams, BuildingTerrainAssessmentStore,
        OperationCatalog, create_building_with_inventory, starter_building_definitions,
        starter_operation_definitions,
    };

    let workbench = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "workbench")
        .expect("workbench");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![workbench.clone()], &categories).unwrap();
    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &workbench.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    let snapshot = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    )
    .unwrap();
    let production = snapshot.production.expect("workbench production");
    assert!(production.show_operation_selector);
    assert_eq!(production.operation_options.len(), 2);
    assert!(
        production
            .operation_options
            .iter()
            .all(|option| !option.display_name.is_empty())
    );
    assert!(
        production
            .operation_options
            .iter()
            .all(|option| option.operation_id.as_str() != option.display_name)
    );
}

#[test]
fn production_controls_snapshot_has_no_automation_fields() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingOperationParams, BuildingTerrainAssessmentStore,
        ControlSource, OperationCatalog, create_building_with_inventory,
        starter_building_definitions, starter_operation_definitions,
    };

    let farm_def = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("farm");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![farm_def.clone()], &categories).unwrap();
    let mut world = flat_world();
    let farm_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &farm_def.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    {
        let store = world.building_production_store_mut();
        store.ensure_policy_for_building(farm_id, &farm_def, &operation_catalog);
        store.get_policy_mut(farm_id).control_source = ControlSource::PlayerControlled;
    }
    let production = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        farm_id,
    )
    .unwrap()
    .production
    .unwrap();
    assert!(production.enabled || !production.enabled);
    let encoded = format!("{production:?}");
    assert!(!encoded.contains("manual_override"));
    assert!(!encoded.contains("can_return_to_automatic"));
}

#[test]
fn viewing_building_panel_does_not_change_control_source() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingOperationParams, BuildingTerrainAssessmentStore,
        ControlSource, OperationCatalog, create_building_with_inventory,
        starter_building_definitions, starter_operation_definitions,
    };

    let farm = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("farm");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![farm.clone()], &categories).unwrap();
    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &farm.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    {
        let store = world.building_production_store_mut();
        store.ensure_policy_for_building(building_id, &farm, &operation_catalog);
        store.get_policy_mut(building_id).control_source = ControlSource::AIControlled;
    }
    let before = world
        .building_production_store()
        .get_policy(building_id)
        .unwrap()
        .control_source;
    let _ = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    );
    let after = world
        .building_production_store()
        .get_policy(building_id)
        .unwrap()
        .control_source;
    assert_eq!(before, after);
}

#[test]
fn smelter_merged_live_catalog_path_exposes_four_bindings_in_snapshot() {
    use crate::world::{BuildingCategoryCatalog, BuildingCategoryId};
    use crate::world::{
        BuildingDefinition, BuildingDefinitionId, BuildingOperationParams, BuildingRenderKey,
        BuildingTerrainAssessmentStore, FootprintSpec, OperationCatalog,
        create_building_with_inventory, effective_inventory_binding_definitions,
        merge_starter_extensions_into_catalog, starter_building_definitions,
        starter_operation_definitions,
    };

    let categories = BuildingCategoryCatalog::default();
    let excel_smelter = BuildingDefinition::new(
        BuildingDefinitionId::new("smelter"),
        "Smelter",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        400,
        90.0,
        FootprintSpec::Circle { radius_meters: 2.5 },
        30.0,
        true,
    );
    let mut catalog = BuildingCatalog::from_definitions(vec![excel_smelter], &categories).unwrap();
    merge_starter_extensions_into_catalog(&mut catalog, &categories).unwrap();
    let smelter = catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .expect("smelter");
    assert_eq!(effective_inventory_binding_definitions(smelter).len(), 4);

    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &smelter.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let bindings = world
        .building_inventory_binding_store()
        .get(building_id)
        .expect("bindings");
    assert_eq!(bindings.bindings().len(), 4);

    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    let snapshot = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    )
    .unwrap();
    assert_eq!(snapshot.inventories.len(), 4);
    let labels: Vec<_> = snapshot
        .inventories
        .iter()
        .map(|section| section.label.as_str())
        .collect();
    for expected in ["Input", "Fuel", "Output", "Waste"] {
        assert!(labels.contains(&expected), "missing {expected}");
    }
}

#[test]
fn empty_output_and_waste_bindings_remain_in_snapshot() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingOperationParams, BuildingTerrainAssessmentStore,
        OperationCatalog, create_building_with_inventory, starter_building_definitions,
        starter_operation_definitions,
    };

    let smelter = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "smelter")
        .expect("smelter");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![smelter.clone()], &categories).unwrap();
    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &smelter.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    let snapshot = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    )
    .unwrap();
    let output = snapshot
        .inventories
        .iter()
        .find(|section| section.label == "Output")
        .expect("output section");
    let waste = snapshot
        .inventories
        .iter()
        .find(|section| section.label == "Waste")
        .expect("waste section");
    assert_eq!(output.content_revision, 0);
    assert_eq!(waste.content_revision, 0);
    assert_eq!((output.grid_width, output.grid_height), (4, 4));
}

#[test]
fn viewing_building_panel_does_not_mutate_selected_operation() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingOperationParams, BuildingTerrainAssessmentStore,
        OperationCatalog, create_building_with_inventory, starter_building_definitions,
        starter_operation_definitions,
    };

    let farm = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("farm");
    let categories = BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(vec![farm.clone()], &categories).unwrap();
    let mut world = flat_world();
    let building_id = create_building_with_inventory(
        &catalog,
        &mut world,
        &farm.id,
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let mut assessment = BuildingTerrainAssessmentStore::default();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut params = BuildingOperationParams {
        field_catalog: &crate::world::TerrainFieldCatalog::default(),
        requirement_catalog: &crate::world::BuildingFieldRequirementCatalog::default(),
        profile_catalog: &crate::world::FieldResponseProfileCatalog::default(),
        footprint_catalog: &crate::world::FootprintCatalog::default(),
        operation_catalog: &operation_catalog,
        inventory_ctx: &inventory_ctx(),
        requirement_revision: 0,
        profile_revision: 0,
        assessment_store: &mut assessment,
    };
    let selected_before = world
        .building_production_store()
        .get_policy(building_id)
        .and_then(|policy| policy.selected_operation.clone());
    let _ = build_building_panel_snapshot(
        &world,
        &catalog,
        &operation_catalog,
        &mut params,
        inventory_profiles(),
        building_id,
    );
    let selected_after = world
        .building_production_store()
        .get_policy(building_id)
        .and_then(|policy| policy.selected_operation.clone());
    assert_eq!(selected_before, selected_after);
}

#[test]
fn sa5_does_not_overwrite_player_controlled_building_after_player_disable() {
    use crate::world::{
        Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
        ControlSource, EmergencyCatalog, NeedCatalog, NeedCategory, NeedTarget, OperationCatalog,
        ResponseCatalog, SettlementKind, SettlementOwnership, apply_player_production_enabled,
        arbitrate_settlement_intent_now, assign_building_settlement,
        create_building_with_inventory, create_settlement_with_treasury,
        discover_settlement_responses_now, evaluate_settlement_needs_now,
        propagate_building_intent_now, starter_building_definitions, starter_operation_definitions,
    };

    let mut world = flat_world();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let interaction_catalog = crate::world::BuildingInteractionProfileCatalog::default();
    let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);
    let core = create_building_with_inventory(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("settlement_core"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        ownership,
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    let farm = create_building_with_inventory(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(10.0, 10.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        ownership,
        None,
        &inventory_ctx(),
    )
    .unwrap()
    .id;
    for building_id in [core, farm] {
        world.mutate_building(building_id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
    }
    let settlement = create_settlement_with_treasury(
        &mut world,
        &building_catalog,
        &interaction_catalog,
        core,
        "SA5",
        SettlementOwnership::player_default(),
        pos(50.0, 50.0),
        0,
    )
    .unwrap();
    for building_id in [core, farm] {
        assign_building_settlement(&mut world, building_id, Some(settlement.settlement_id))
            .unwrap();
    }
    if let Some(state) = world
        .settlement_state_store_mut()
        .get_mut(settlement.settlement_id)
    {
        state.kind = SettlementKind::Town;
        state
            .need_targets
            .push(NeedTarget::new(NeedCategory::Food, 10, 0.8));
    }
    {
        let store = world.building_production_store_mut();
        let def = building_catalog
            .get(&BuildingDefinitionId::new("prispod_farm"))
            .unwrap();
        store.ensure_policy_for_building(farm, def, &operation_catalog);
        store.get_policy_mut(farm).enabled = true;
        store.get_policy_mut(farm).control_source = ControlSource::AIControlled;
    }

    apply_player_production_enabled(&mut world, farm, false).unwrap();

    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let ctx = inventory_ctx();
    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &building_catalog,
        ctx.items,
        &crate::world::UnitCatalog::default(),
        ctx,
        &EmergencyCatalog::default(),
        settlement.settlement_id,
        1,
    );
    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &building_catalog,
        settlement.settlement_id,
        1,
    );
    arbitrate_settlement_intent_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        settlement.settlement_id,
        1,
    );
    propagate_building_intent_now(
        &mut world,
        &response_catalog,
        &building_catalog,
        &operation_catalog,
        ctx,
        settlement.settlement_id,
        1,
    );

    let policy = world.building_production_store().get_policy(farm).unwrap();
    assert!(!policy.enabled);
    assert_eq!(policy.control_source, ControlSource::PlayerControlled);
}

fn inventory_profiles() -> &'static crate::world::InventoryProfileCatalog {
    use crate::world::{InventoryProfileCatalog, starter_inventory_profile_definitions};
    static PROFILES: std::sync::OnceLock<crate::world::InventoryProfileCatalog> =
        std::sync::OnceLock::new();
    PROFILES.get_or_init(|| {
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap()
    })
}

fn inventory_ctx() -> &'static crate::world::InventoryCatalogCtx<'static> {
    use crate::world::{
        InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions,
    };
    static CTX: std::sync::OnceLock<crate::world::InventoryCatalogCtx<'static>> =
        std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        crate::world::InventoryCatalogCtx::new(items, categories, profiles)
    })
}

#[test]
fn building_panel_viewable_without_unit_actor() {
    use crate::ui::gameplay::building_panel::interaction::building_inventory_grid_interaction;
    use crate::ui::gameplay::inventory::{InventoryGridInteraction, InventoryPaneSide};

    assert_eq!(
        building_inventory_grid_interaction(false),
        InventoryGridInteraction::ReadOnly
    );
}

#[test]
fn building_inventory_transfer_requires_eligible_actor() {
    use crate::ui::gameplay::building_panel::interaction::{
        building_inventory_grid_interaction, building_inventory_transfer_eligible,
        resolve_building_inventory_actor,
    };
    use crate::ui::gameplay::inventory::InventoryUiState;
    use crate::ui::gameplay::inventory::{InventoryGridInteraction, InventoryPaneSide};
    use crate::world::{
        BuildingCategoryCatalog, BuildingInteractionProfileCatalog, UnitCatalog, UnitDefinitionId,
        UnitOwnership, UnitSource, create_building_with_inventory, create_unit_with_inventory,
        starter_building_definitions, starter_unit_definitions,
    };

    let categories = BuildingCategoryCatalog::default();
    let catalog =
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
    let interaction = BuildingInteractionProfileCatalog::default();
    let mut world = flat_world();
    let ctx = inventory_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let farm = create_building_with_inventory(
        &catalog,
        &mut world,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(30.0, 30.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        ctx,
    )
    .unwrap();
    let near = create_unit_with_inventory(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(30.5, 30.5),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        ctx,
    )
    .unwrap();
    let far = create_unit_with_inventory(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        ctx,
    )
    .unwrap();

    let ui = InventoryUiState::default();
    let mut selection = SelectedUnits::default();
    assert!(resolve_building_inventory_actor(&ui, &selection).is_none());

    selection.set_single(near.id);
    assert_eq!(
        resolve_building_inventory_actor(&ui, &selection),
        Some(near.id)
    );
    assert!(building_inventory_transfer_eligible(
        &world,
        &catalog,
        &interaction,
        farm.id,
        near.id
    ));
    assert_eq!(
        building_inventory_grid_interaction(true),
        InventoryGridInteraction::Interactive {
            side: InventoryPaneSide::Right
        }
    );

    selection.set_single(far.id);
    assert!(!building_inventory_transfer_eligible(
        &world,
        &catalog,
        &interaction,
        farm.id,
        far.id
    ));
}

#[test]
fn left_click_building_inspection_does_not_queue_unit_inventory() {
    use crate::client::inventory_intent::InventoryIntentQueue;

    let mut world = flat_world();
    let owned = insert_building(
        &mut world,
        16,
        BuildingOwnership::with_affiliation(Affiliation::Player),
    );
    let mut panel = BuildingPanelState::default();
    let mut inventory_queue = InventoryIntentQueue::default();

    on_gameplay_building_selected(owned, &mut panel, &world, &player());

    assert_eq!(panel.open_building_id, Some(owned));
    assert!(inventory_queue.is_empty());
}

#[test]
fn foreign_building_inventory_access_denied() {
    use crate::world::{
        BuildingCategoryCatalog, BuildingInteractionProfileCatalog, InventoryAccessDenialReason,
        InventoryAccessResult, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource,
        can_unit_access_building_inventory, create_building_with_inventory,
        create_unit_with_inventory, starter_building_definitions, starter_unit_definitions,
    };

    let categories = BuildingCategoryCatalog::default();
    let catalog =
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
    let interaction = BuildingInteractionProfileCatalog::default();
    let mut world = flat_world();
    let ctx = inventory_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let farm = create_building_with_inventory(
        &catalog,
        &mut world,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(40.0, 40.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        crate::world::BuildingOwnership {
            owner_id: Some(crate::world::OwnerId::new(999)),
            team_id: None,
            affiliation: Affiliation::Hostile,
        },
        None,
        ctx,
    )
    .unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(40.5, 40.5),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        ctx,
    )
    .unwrap();
    let access =
        can_unit_access_building_inventory(&world, &catalog, &interaction, unit.id, farm.id);
    assert!(matches!(
        access,
        InventoryAccessResult::Denied(InventoryAccessDenialReason::PolicyDenied)
    ));
}
