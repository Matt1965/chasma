//! EP2 production runtime integration tests.

use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
use crate::world::building::operation::{
    BuildingOperationParams, OperationLifecycle, RepeatMode, apply_operation_ticks,
    reset_production_progress, set_production_enabled, set_production_paused,
    set_production_repeat_count, step_workstation_operation, validate_production_runtime,
    ProductionValidationIssue, PRODUCTION_STEPPING_MODEL,
};
use crate::world::building::operational_efficiency::OperationalLimitingFactor;
use crate::world::building::terrain_assessment::{
    BuildingTerrainAssessmentStore, TerrainAssessmentCatalogs,
};
use crate::world::{
    BuildingCategoryCatalog, BuildingDefinition, BuildingDefinitionId, BuildingId,
    BuildingLifecycleState, BuildingOwnership, BuildingPlacement, BuildingRecord,
    BuildingRenderKey, BuildingSource, ChunkCoord, ChunkExtent, ChunkId, DoodadCatalog,
    FootprintCatalog, FootprintSpec, InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, LocalPosition, OccupancyCatalogs, UnitCatalog, UnitDefinitionId, UnitId,
    UnitSource, WorldData, WorldPosition, create_unit, destroy_building,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions,
};
use bevy::prelude::{Quat, Vec3};

fn flat_world() -> WorldData {
    let layout = crate::world::WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    });
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn workbench_record(building_id: BuildingId) -> BuildingRecord {
    let mut record = BuildingRecord::new(
        building_id,
        BuildingDefinitionId::new("workbench"),
        BuildingPlacement::new(pos(64.0, 64.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        200,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Complete;
    record.construction.progress_0_1 = 1.0;
    record
}

fn workbench_catalogs() -> (
    TerrainAssessmentCatalogs<'static>,
    crate::world::BuildingCatalog,
) {
    let field_catalog = crate::world::TerrainFieldCatalog::default();
    let profile_catalog = crate::world::FieldResponseProfileCatalog::default();
    let requirement_catalog = crate::world::BuildingFieldRequirementCatalog::default();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog = crate::world::BuildingCatalog::from_definitions(
        vec![
            BuildingDefinition::new(
                BuildingDefinitionId::new("workbench"),
                "Workbench",
                crate::world::BuildingCategoryId::new("production"),
                BuildingRenderKey::reserved("workbench"),
                BuildingRenderKey::reserved("workbench_collision"),
                200,
                30.0,
                FootprintSpec::Circle { radius_meters: 1.5 },
                30.0,
                true,
            )
            .with_supported_operations([crate::world::OperationDefinitionId::new(
                "test_workbench_op",
            )])
            .with_default_operation_id(crate::world::OperationDefinitionId::new(
                "test_workbench_op",
            )),
        ],
        &categories,
    )
    .unwrap();
    let footprint_catalog = FootprintCatalog::default();
    let catalogs = TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(requirement_catalog)),
        profiles: Box::leak(Box::new(profile_catalog)),
        fields: Box::leak(Box::new(field_catalog)),
        footprints: Box::leak(Box::new(footprint_catalog)),
        requirement_revision: 0,
        profile_revision: 0,
    };
    (catalogs, building_catalog)
}

fn setup_workbench_world() -> (
    WorldData,
    BuildingId,
    UnitId,
    TerrainAssessmentCatalogs<'static>,
    crate::world::BuildingCatalog,
    crate::world::OperationCatalog,
) {
    let mut world = flat_world();
    let building_id = BuildingId::new(1);
    world
        .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), workbench_record(building_id))
        .unwrap();
    let unit_catalog = UnitCatalog::default();
    let worker = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(64.0, 64.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    let (catalogs, building_catalog) = workbench_catalogs();
    let operation_catalog = test_operation_catalog();
    (
        world,
        building_id,
        worker,
        catalogs,
        building_catalog,
        operation_catalog,
    )
}

fn test_inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items =
            ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn operation_params<'a>(
    catalogs: &'a TerrainAssessmentCatalogs<'static>,
    assessment_store: &'a mut BuildingTerrainAssessmentStore,
    operation_catalog: &'a crate::world::OperationCatalog,
) -> BuildingOperationParams<'a> {
    BuildingOperationParams {
        field_catalog: catalogs.fields,
        requirement_catalog: catalogs.requirements,
        profile_catalog: catalogs.profiles,
        footprint_catalog: catalogs.footprints,
        operation_catalog,
        inventory_ctx: test_inventory_ctx(),
        requirement_revision: catalogs.requirement_revision,
        profile_revision: catalogs.profile_revision,
        assessment_store,
    }
}

fn test_operation_catalog() -> crate::world::OperationCatalog {
    crate::world::OperationCatalog::from_definitions(vec![
        crate::world::test_workbench_operation(),
    ])
    .unwrap()
}

fn second_worker(world: &mut WorldData) -> UnitId {
    let unit_catalog = UnitCatalog::default();
    create_unit(
        &unit_catalog,
        world,
        &UnitDefinitionId::new("wolf"),
        pos(64.0, 64.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id
}

fn total_inventory_entries(world: &WorldData) -> usize {
    world
        .inventory_store()
        .sorted_inventory_ids()
        .iter()
        .filter_map(|id| world.inventory_store().get(*id))
        .map(|inv| inv.placed_entries().len())
        .sum()
}

#[test]
fn worker_operating_building_contributes_labor() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert!(report.can_operate);
    assert_eq!(report.lifecycle, OperationLifecycle::Running);
    assert!(report.scaled_progress > 0);
}

#[test]
fn production_progress_owned_by_building_not_worker() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    let progress = world
        .building_production_store()
        .get_state(building_id)
        .unwrap()
        .progress
        .value();
    assert!(progress > 0);
}

#[test]
fn interrupting_worker_preserves_building_progress() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    let saved = world
        .building_production_store()
        .get_state(building_id)
        .unwrap()
        .progress
        .value();
    let after = world
        .building_production_store()
        .get_state(building_id)
        .unwrap()
        .progress
        .value();
    assert_eq!(saved, after);
}

#[test]
fn second_worker_resumes_same_building_operation() {
    let (mut world, building_id, worker1, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let worker2 = second_worker(&mut world);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker1,
    )
    .unwrap();
    let partial = world
        .building_production_store()
        .get_state(building_id)
        .unwrap()
        .progress
        .value();
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker2,
    )
    .unwrap();
    let resumed = world
        .building_production_store()
        .get_state(building_id)
        .unwrap()
        .progress
        .value();
    assert!(resumed > partial);
}

#[test]
fn disabled_buildings_do_not_advance() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    set_production_enabled(&mut world, building_id, false).unwrap();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert!(!report.can_operate);
    assert_eq!(report.lifecycle, OperationLifecycle::Disabled);
    assert_eq!(
        world
            .building_production_store()
            .get_state(building_id)
            .unwrap()
            .progress
            .value(),
        0
    );
}

#[test]
fn paused_buildings_do_not_advance() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    set_production_paused(&mut world, building_id, true).unwrap();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert!(!report.can_operate);
    assert_eq!(report.limiting_factor, OperationalLimitingFactor::Paused);
    assert_eq!(
        world
            .building_production_store()
            .get_state(building_id)
            .unwrap()
            .progress
            .value(),
        0
    );
}

#[test]
fn continuous_mode_cycles_without_producing_items() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let before_items = total_inventory_entries(&world);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let ticks = crate::world::building::operation::expected_ticks_to_complete(
        EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT,
    );
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
        ticks as u32,
    )
    .unwrap();
    let state = world.building_production_store().get_state(building_id).unwrap();
    assert!(state.completion_count >= 1);
    assert_eq!(state.lifecycle, OperationLifecycle::Running);
    assert_eq!(total_inventory_entries(&world), before_items);
}

#[test]
fn repeat_count_stops_after_configured_completions() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("workbench"))
        .expect("workbench definition");
    world
        .building_production_store_mut()
        .ensure_policy_for_building(building_id, definition, &operation_catalog);
    set_production_repeat_count(&mut world, building_id, 1).unwrap();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let ticks = crate::world::building::operation::expected_ticks_to_complete(
        EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT,
    );
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
        ticks as u32,
    )
    .unwrap();
    let state = world.building_production_store().get_state(building_id).unwrap();
    assert_eq!(state.lifecycle, OperationLifecycle::Completed);
    assert!(state.completion_count >= 1);
}

#[test]
fn completion_count_and_progress_survive_save_load() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
        10,
    )
    .unwrap();
    let exported = world.building_production_store().export_save_state();
    let mut restored = WorldData::new(crate::world::WorldConfig::default().chunk_layout());
    restored
        .building_production_store_mut()
        .import_save_state(exported);
    assert_eq!(
        restored.building_production_store().get_state(building_id),
        world.building_production_store().get_state(building_id)
    );
}

#[test]
fn policy_survives_save_load() {
    let (mut world, building_id, _worker, _catalogs, _building_catalog, _operation_catalog) =
        setup_workbench_world();
    world
        .building_production_store_mut()
        .get_policy_mut(building_id)
        .repeat_mode = RepeatMode::Count(3);
    world
        .building_production_store_mut()
        .get_policy_mut(building_id)
        .paused = true;
    let exported = world.building_production_store().export_save_state();
    let mut restored = WorldData::new(crate::world::WorldConfig::default().chunk_layout());
    restored
        .building_production_store_mut()
        .import_save_state(exported);
    let policy = restored
        .building_production_store()
        .get_policy(building_id)
        .unwrap();
    assert_eq!(policy.repeat_mode, RepeatMode::Count(3));
    assert!(policy.paused);
}

#[test]
fn building_removal_cleans_production_state() {
    let (mut world, building_id, _worker, _catalogs, building_catalog, _operation_catalog) =
        setup_workbench_world();
    world
        .building_production_store_mut()
        .get_state_mut(building_id)
        .progress = crate::world::building::operation::ProductionProgress(42);
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occ = OccupancyCatalogs {
        doodad: &doodad,
        building: &building_catalog,
        footprint: &footprint,
    };
    let _ = destroy_building(
        &mut world,
        &building_catalog,
        &doodad,
        occ,
        building_id,
        "test",
        None,
    );
    assert!(world.building_production_store().get_state(building_id).is_none());
    assert!(world.building_production_store().get_policy(building_id).is_none());
}

#[test]
fn absent_selected_operation_is_handled_safely() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    world
        .building_production_store_mut()
        .get_policy_mut(building_id)
        .selected_operation = None;
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert!(!report.can_operate);
    assert_eq!(report.limiting_factor, OperationalLimitingFactor::InvalidOperation);
    assert_eq!(
        world
            .building_production_store()
            .get_state(building_id)
            .unwrap()
            .lifecycle,
        OperationLifecycle::Idle
    );
}

#[test]
fn multiple_workers_contribute_to_one_operation() {
    let (mut world, building_id, worker1, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let worker2 = second_worker(&mut world);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker1,
    )
    .unwrap();
    let single = world
        .building_production_store()
        .get_state(building_id)
        .unwrap()
        .progress
        .value();
    reset_production_progress(&mut world, building_id).unwrap();
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker1,
    )
    .unwrap();
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker2,
    )
    .unwrap();
    let combined = world
        .building_production_store()
        .get_state(building_id)
        .unwrap()
        .progress
        .value();
    assert!(combined > single);
}

#[test]
fn stepping_one_building_does_not_mutate_other_buildings() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let other_id = BuildingId::new(2);
    world
        .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), workbench_record(other_id))
        .unwrap();
    world
        .building_production_store_mut()
        .ensure_policy_for_building(
            other_id,
            building_catalog
                .get(&BuildingDefinitionId::new("workbench"))
                .unwrap(),
            &operation_catalog,
        );
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    let other = world
        .building_production_store()
        .get_state(other_id)
        .map(|state| state.progress.value())
        .unwrap_or(0);
    assert_eq!(other, 0);
    assert_eq!(PRODUCTION_STEPPING_MODEL, "worker-task-driven");
}

#[test]
fn validation_detects_orphaned_production_state() {
    let (mut world, building_id, _worker, _catalogs, _building_catalog, _operation_catalog) =
        setup_workbench_world();
    world.building_production_store_mut().get_state_mut(building_id);
    world.remove_building_by_id(building_id);
    let issues = validate_production_runtime(&world);
    assert!(issues.iter().any(|issue| matches!(
        issue,
        ProductionValidationIssue::OrphanedState { building_id: id } if *id == building_id
    )));
}

#[test]
fn reset_progress_command_clears_runtime_state() {
    let (mut world, building_id, worker, catalogs, building_catalog, operation_catalog) =
        setup_workbench_world();
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &operation_catalog);
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
        5,
    )
    .unwrap();
    reset_production_progress(&mut world, building_id).unwrap();
    let state = world.building_production_store().get_state(building_id).unwrap();
    assert_eq!(state.progress.value(), 0);
    assert_eq!(state.completion_count, 0);
    assert_eq!(state.lifecycle, OperationLifecycle::Idle);
}
