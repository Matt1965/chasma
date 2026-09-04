//! Prispod Farm closed-loop production tests.

use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
use crate::world::building::inventory::attach_inventory_on_building_create;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operation::{
    BuildingOperationParams, FarmProductionPhase, PRODUCTION_PROGRESS_ONE_UNIT,
    apply_operation_ticks, expected_ticks_to_complete, farm_needs_harvest_worker,
    grow_prispods_operation_id, is_prispod_farm_definition, step_all_farm_passive_growth,
};
use crate::world::building::terrain_assessment::TerrainAssessmentCatalogs;
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item};
use crate::world::operation::OperationCatalog;
use crate::world::task::{TaskState, TaskType, sync_operate_workstation_tasks};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinition, BuildingId, BuildingLifecycleState,
    BuildingOwnership, BuildingPlacement, BuildingRecord, BuildingSource, ChunkCoord, ChunkExtent,
    ChunkId, FootprintCatalog, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
    ItemDefinitionId, LocalPosition, TerrainFieldCatalog, TerrainFieldId, UnitCatalog,
    UnitDefinitionId, UnitId, UnitSource, WorldData, WorldPosition, bootstrap_constant_field,
    create_unit, field_value_from_percent, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_operation_definitions,
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

fn test_inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
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
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn prispod_farm_definition() -> BuildingDefinition {
    starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("starter prispod_farm")
}

fn operation_catalog() -> OperationCatalog {
    OperationCatalog::from_definitions(starter_operation_definitions()).unwrap()
}

fn terrain_catalogs(
    building_catalog: &crate::world::BuildingCatalog,
) -> TerrainAssessmentCatalogs<'static> {
    TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(
            crate::world::BuildingFieldRequirementCatalog::default(),
        )),
        profiles: Box::leak(Box::new(
            crate::world::FieldResponseProfileCatalog::default(),
        )),
        fields: Box::leak(Box::new(TerrainFieldCatalog::default())),
        footprints: Box::leak(Box::new(FootprintCatalog::default())),
        requirement_revision: 0,
        profile_revision: 0,
    }
}

fn operation_params<'a>(
    catalogs: &'a TerrainAssessmentCatalogs<'a>,
    assessment_store: &'a mut crate::world::BuildingTerrainAssessmentStore,
    catalog: &'a OperationCatalog,
) -> BuildingOperationParams<'a> {
    BuildingOperationParams {
        field_catalog: catalogs.fields,
        requirement_catalog: catalogs.requirements,
        profile_catalog: catalogs.profiles,
        footprint_catalog: catalogs.footprints,
        operation_catalog: catalog,
        inventory_ctx: test_inventory_ctx(),
        requirement_revision: catalogs.requirement_revision,
        profile_revision: catalogs.profile_revision,
        assessment_store,
    }
}

fn place_farm(
    world: &mut WorldData,
    definition: &BuildingDefinition,
    building_id: BuildingId,
    position: WorldPosition,
) {
    let mut record = BuildingRecord::new(
        building_id,
        definition.id.clone(),
        BuildingPlacement::new(position, Quat::IDENTITY),
        BuildingOwnership::with_affiliation(Affiliation::Player),
        definition.max_hp,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Complete;
    record.construction.progress_0_1 = 1.0;
    attach_inventory_on_building_create(world, test_inventory_ctx(), &mut record, definition)
        .unwrap();
    world
        .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();
}

fn enable_farm(
    world: &mut WorldData,
    building_id: BuildingId,
    definition: &BuildingDefinition,
    ops: &OperationCatalog,
) {
    let store = world.building_production_store_mut();
    store.ensure_policy_for_building(building_id, definition, ops);
    store.get_policy_mut(building_id).enabled = true;
    store.farm_state_mut(building_id);
}

fn setup_farm(
    field_percent: f32,
) -> (
    WorldData,
    BuildingId,
    UnitId,
    TerrainAssessmentCatalogs<'static>,
    crate::world::BuildingCatalog,
    OperationCatalog,
) {
    let mut world = flat_world();
    bootstrap_constant_field(
        world.terrain_fields_mut(),
        TerrainFieldId::new("water"),
        ChunkCoord::new(0, 0),
        field_value_from_percent(field_percent),
    );
    let definition = prispod_farm_definition();
    let building_id = world.allocate_building_id();
    place_farm(&mut world, &definition, building_id, pos(64.0, 64.0));
    let unit_catalog = UnitCatalog::default();
    let worker_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(64.0, 63.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    let ops = operation_catalog();
    enable_farm(&mut world, building_id, &definition, &ops);
    let catalogs = terrain_catalogs(&building_catalog);
    (
        world,
        building_id,
        worker_id,
        catalogs,
        building_catalog,
        ops,
    )
}

fn binding_inventory(world: &WorldData, building_id: BuildingId) -> crate::world::InventoryId {
    world
        .building_inventory_binding_store()
        .resolve_inventory(
            building_id,
            &BuildingInventoryBindingId::new("primary_output"),
        )
        .expect("primary_output binding")
}

fn grow_farm_to_ready(
    world: &mut WorldData,
    building_catalog: &crate::world::BuildingCatalog,
    catalogs: &TerrainAssessmentCatalogs<'static>,
    ops: &OperationCatalog,
    building_id: BuildingId,
) {
    let mut assessment_store = crate::world::BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(catalogs, &mut assessment_store, ops);
    let ticks = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
    for _ in 0..ticks {
        step_all_farm_passive_growth(world, building_catalog, &mut params);
    }
}

#[test]
fn operational_farm_grows_without_worker() {
    let (mut world, building_id, _worker, catalogs, building_catalog, ops) = setup_farm(50.0);
    let mut assessment_store = crate::world::BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let before = world
        .building_production_store()
        .farm_state(building_id)
        .unwrap()
        .growth_progress
        .value();
    step_all_farm_passive_growth(&mut world, &building_catalog, &mut params);
    let after = world
        .building_production_store()
        .farm_state(building_id)
        .unwrap()
        .growth_progress
        .value();
    assert!(after > before);
}

#[test]
fn invalid_environment_prevents_growth() {
    let (mut world, building_id, _worker, catalogs, building_catalog, ops) = setup_farm(10.0);
    let mut assessment_store = crate::world::BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    for _ in 0..200 {
        step_all_farm_passive_growth(&mut world, &building_catalog, &mut params);
    }
    let farm = world
        .building_production_store()
        .farm_state(building_id)
        .unwrap();
    assert_eq!(farm.phase, FarmProductionPhase::Growing);
    assert!(farm.growth_progress.value() < PRODUCTION_PROGRESS_ONE_UNIT / 2);
}

#[test]
fn full_growth_enters_ready_without_output() {
    let (mut world, building_id, _worker, catalogs, building_catalog, ops) = setup_farm(50.0);
    grow_farm_to_ready(&mut world, &building_catalog, &catalogs, &ops, building_id);
    let farm = world
        .building_production_store()
        .farm_state(building_id)
        .unwrap();
    assert_eq!(farm.phase, FarmProductionPhase::ReadyToHarvest);
    let inventory_id = binding_inventory(&world, building_id);
    assert_eq!(
        count_stack_item(
            world.inventory_store().get(inventory_id).unwrap(),
            &ItemDefinitionId::new("prispod"),
        ),
        0
    );
}

#[test]
fn ready_farm_exposes_harvest_work_task() {
    let (mut world, building_id, _worker, catalogs, building_catalog, ops) = setup_farm(50.0);
    grow_farm_to_ready(&mut world, &building_catalog, &catalogs, &ops, building_id);
    let definition = prispod_farm_definition();
    assert!(farm_needs_harvest_worker(
        world.building_production_store(),
        building_id,
        &definition,
    ));
    sync_operate_workstation_tasks(&mut world, &building_catalog, 1);
    let has_operate = world
        .task_store()
        .building_task_ids(building_id)
        .iter()
        .any(|task_id| {
            world
                .task_store()
                .get(*task_id)
                .is_some_and(|task| task.task_type == TaskType::OperateWorkstation)
        });
    assert!(has_operate);
}

#[test]
fn worker_harvest_places_prispod_and_resets_growth() {
    let (mut world, building_id, worker_id, catalogs, building_catalog, ops) = setup_farm(50.0);
    grow_farm_to_ready(&mut world, &building_catalog, &catalogs, &ops, building_id);
    let mut assessment_store = crate::world::BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let ticks = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker_id,
        ticks,
    )
    .unwrap();
    let inventory_id = binding_inventory(&world, building_id);
    assert_eq!(
        count_stack_item(
            world.inventory_store().get(inventory_id).unwrap(),
            &ItemDefinitionId::new("prispod"),
        ),
        1
    );
    let farm = world
        .building_production_store()
        .farm_state(building_id)
        .unwrap();
    assert_eq!(farm.phase, FarmProductionPhase::Growing);
    assert_eq!(farm.growth_progress.value(), 0);
}

#[test]
fn output_full_preserves_ready_crop() {
    let (mut world, building_id, worker_id, catalogs, building_catalog, ops) = setup_farm(50.0);
    grow_farm_to_ready(&mut world, &building_catalog, &catalogs, &ops, building_id);
    let inventory_id = binding_inventory(&world, building_id);
    let (store, instances) = world.inventory_runtime_mut();
    crate::world::place_stack_first_fit(
        store,
        instances,
        test_inventory_ctx(),
        inventory_id,
        ItemDefinitionId::new("prispod"),
        1,
    )
    .unwrap();
    let mut assessment_store = crate::world::BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let ticks = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker_id,
        ticks,
    )
    .unwrap();
    let farm = world
        .building_production_store()
        .farm_state(building_id)
        .unwrap();
    assert_eq!(farm.phase, FarmProductionPhase::ReadyToHarvest);
    assert_eq!(
        count_stack_item(
            world.inventory_store().get(inventory_id).unwrap(),
            &ItemDefinitionId::new("prispod"),
        ),
        1
    );
}

#[test]
fn clearing_output_allows_harvest_completion() {
    let (mut world, building_id, worker_id, catalogs, building_catalog, ops) = setup_farm(50.0);
    grow_farm_to_ready(&mut world, &building_catalog, &catalogs, &ops, building_id);
    let inventory_id = binding_inventory(&world, building_id);
    let (store, instances) = world.inventory_runtime_mut();
    crate::world::place_stack_first_fit(
        store,
        instances,
        test_inventory_ctx(),
        inventory_id,
        ItemDefinitionId::new("prispod"),
        1,
    )
    .unwrap();
    let mut assessment_store = crate::world::BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let ticks = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker_id,
        ticks,
    )
    .unwrap();
    if let Some(record) = world.inventory_store().get(inventory_id) {
        if !record.placed_entries().is_empty() {
            let (store, instances) = world.inventory_runtime_mut();
            let _ =
                crate::world::remove_entry(store, instances, test_inventory_ctx(), inventory_id, 0);
        }
    }
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker_id,
        ticks,
    )
    .unwrap();
    assert_eq!(
        count_stack_item(
            world.inventory_store().get(inventory_id).unwrap(),
            &ItemDefinitionId::new("prispod"),
        ),
        1
    );
}

#[test]
fn growing_farm_does_not_emit_operate_tasks() {
    let (mut world, building_id, _worker, _catalogs, building_catalog, _ops) = setup_farm(50.0);
    sync_operate_workstation_tasks(&mut world, &building_catalog, 1);
    let operate_available = world
        .task_store()
        .building_task_ids(building_id)
        .iter()
        .any(|task_id| {
            world.task_store().get(*task_id).is_some_and(|task| {
                task.task_type == TaskType::OperateWorkstation && task.state == TaskState::Available
            })
        });
    assert!(!operate_available);
}

#[test]
fn prispod_farm_definition_is_farm() {
    let farm = prispod_farm_definition();
    assert!(is_prispod_farm_definition(&farm));
}

#[test]
fn farm_operational_efficiency_passes_on_rich_water_field() {
    use crate::world::building::operational_efficiency::OperationalLimitingFactor;
    use crate::world::building_operational_efficiency;

    let (world, building_id, _, catalogs, building_catalog, ops) = setup_farm(80.0);
    let mut assessment_store = crate::world::BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let grow_op = ops
        .get(&grow_prispods_operation_id())
        .expect("grow_prispods");
    let mut ctx = params.efficiency_context(&world, &building_catalog);
    let report =
        building_operational_efficiency(&mut ctx, building_id, Some(grow_op)).expect("efficiency");
    assert!(report.can_operate);
    assert_ne!(
        report.limiting_factor,
        OperationalLimitingFactor::TerrainAverageBelowMinimum(crate::world::TerrainFieldId::new(
            "water"
        ))
    );
}
