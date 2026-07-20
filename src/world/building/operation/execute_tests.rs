//! EP5 generic production execution tests.

use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
use crate::world::building::inventory::attach_inventory_on_building_create;
use crate::world::building::inventory_binding::{
    BuildingInventoryBindingDefinition, BuildingInventoryBindingId, BuildingInventoryRole,
};
use crate::world::building::operation::{
    BuildingOperationParams, OperationLifecycle, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    RepeatMode, apply_operation_ticks, assess_production_execution, execute_production_cycle,
    set_production_repeat_count, step_workstation_operation,
};
use crate::world::inventory::count_stack_item;
use crate::world::building::operational_efficiency::OperationalLimitingFactor;
use crate::world::building::terrain_assessment::{
    BuildingTerrainAssessmentStore, TerrainAssessmentCatalogs,
};
use crate::world::inventory::{InventoryCatalogCtx, InventoryEntryContents, place_stack_first_fit};
use crate::world::operation::OperationCatalog;
use crate::world::{
    BuildingCategoryCatalog, BuildingDefinition, BuildingDefinitionId, BuildingId,
    BuildingLifecycleState, BuildingOwnership, BuildingPlacement, BuildingRecord,
    BuildingRenderKey, BuildingSource, ChunkCoord, ChunkExtent, ChunkId, FootprintCatalog,
    FootprintSpec, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId,
    LocalPosition, OperationDefinitionId, UnitCatalog, UnitDefinitionId, UnitId, UnitSource,
    WorldData, WorldPosition, bootstrap_constant_field, create_unit, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_operation_definitions,
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

fn operation_catalog() -> OperationCatalog {
    OperationCatalog::from_definitions(starter_operation_definitions()).unwrap()
}

fn smelter_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("smelter"),
        "Smelter",
        crate::world::BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        400,
        90.0,
        FootprintSpec::Circle { radius_meters: 2.5 },
        30.0,
        true,
    )
    .with_supported_operations([OperationDefinitionId::new("smelt_iron")])
    .with_default_operation_id(OperationDefinitionId::new("smelt_iron"))
    .with_inventory_bindings(vec![
        BuildingInventoryBindingDefinition::new(
            "ore_input",
            BuildingInventoryRole::Input,
            crate::world::InventoryProfileId::new("chest_large"),
        ),
        BuildingInventoryBindingDefinition::new(
            "fuel_input",
            BuildingInventoryRole::Fuel,
            crate::world::InventoryProfileId::new("chest_small"),
        ),
        BuildingInventoryBindingDefinition::new(
            "metal_output",
            BuildingInventoryRole::Output,
            crate::world::InventoryProfileId::new("chest_small"),
        ),
        BuildingInventoryBindingDefinition::new(
            "slag_output",
            BuildingInventoryRole::Waste,
            crate::world::InventoryProfileId::new("chest_small"),
        ),
    ])
}

fn iron_mine_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("iron_mine"),
        "Iron Mine",
        crate::world::BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        400,
        90.0,
        FootprintSpec::Circle { radius_meters: 2.5 },
        30.0,
        true,
    )
    .with_supported_operations([OperationDefinitionId::new("mine_iron")])
    .with_default_operation_id(OperationDefinitionId::new("mine_iron"))
    .with_inventory_bindings(vec![BuildingInventoryBindingDefinition::new(
        "primary_output",
        BuildingInventoryRole::Output,
        crate::world::InventoryProfileId::new("chest_large"),
    )
    .with_default(true)])
}

fn terrain_catalogs(building_catalog: &crate::world::BuildingCatalog) -> TerrainAssessmentCatalogs<'static> {
    let field_catalog = crate::world::TerrainFieldCatalog::default();
    let profile_catalog = crate::world::FieldResponseProfileCatalog::default();
    let requirement_catalog = crate::world::BuildingFieldRequirementCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(requirement_catalog)),
        profiles: Box::leak(Box::new(profile_catalog)),
        fields: Box::leak(Box::new(field_catalog)),
        footprints: Box::leak(Box::new(footprint_catalog)),
        requirement_revision: 0,
        profile_revision: 0,
    }
}

fn operation_params<'a>(
    catalogs: &'a TerrainAssessmentCatalogs<'a>,
    assessment_store: &'a mut BuildingTerrainAssessmentStore,
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

fn place_building(
    world: &mut WorldData,
    definition: &BuildingDefinition,
    building_id: BuildingId,
) {
    let mut record = BuildingRecord::new(
        building_id,
        definition.id.clone(),
        BuildingPlacement::new(pos(64.0, 64.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        400,
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

fn worker(world: &mut WorldData) -> UnitId {
    let unit_catalog = UnitCatalog::default();
    create_unit(
        &unit_catalog,
        world,
        &UnitDefinitionId::new("wolf"),
        pos(64.0, 63.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id
}

fn binding_inventory(
    world: &WorldData,
    building_id: BuildingId,
    binding: &str,
) -> crate::world::InventoryId {
    world
        .building_inventory_binding_store()
        .resolve_inventory(
            building_id,
            &BuildingInventoryBindingId::new(binding),
        )
        .expect("binding")
}

fn count_item(world: &WorldData, inventory_id: crate::world::InventoryId, item: &str) -> u32 {
    world
        .inventory_store()
        .get(inventory_id)
        .map(|record| count_stack_item(record, &ItemDefinitionId::new(item)))
        .unwrap_or(0)
}

fn stock_smelter_inputs(world: &mut WorldData, building_id: BuildingId) {
    let ore = binding_inventory(world, building_id, "ore_input");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        test_inventory_ctx(),
        ore,
        ItemDefinitionId::new("iron_ore"),
        10,
    )
    .unwrap();
}

fn setup_smelter() -> (
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
        crate::world::TerrainFieldId::new("iron"),
        ChunkCoord::new(0, 0),
        crate::world::field_value_from_percent(100.0),
    );
    let building_id = world.allocate_building_id();
    let definition = smelter_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    place_building(&mut world, &definition, building_id);
    stock_smelter_inputs(&mut world, building_id);
    let worker_id = worker(&mut world);
    let catalogs = terrain_catalogs(&building_catalog);
    let ops = operation_catalog();
    (
        world,
        building_id,
        worker_id,
        catalogs,
        building_catalog,
        ops,
    )
}

fn setup_iron_mine() -> (
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
        crate::world::TerrainFieldId::new("iron"),
        ChunkCoord::new(0, 0),
        crate::world::field_value_from_percent(100.0),
    );
    let building_id = world.allocate_building_id();
    let definition = iron_mine_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    place_building(&mut world, &definition, building_id);
    let worker_id = worker(&mut world);
    let catalogs = terrain_catalogs(&building_catalog);
    let ops = operation_catalog();
    (
        world,
        building_id,
        worker_id,
        catalogs,
        building_catalog,
        ops,
    )
}

fn complete_one_cycle(
    world: &mut WorldData,
    params: &mut BuildingOperationParams<'_>,
    building_catalog: &crate::world::BuildingCatalog,
    building_id: BuildingId,
    worker_id: UnitId,
) {
    let ticks = crate::world::building::operation::expected_ticks_to_complete(
        EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT,
    ) as u32;
    let _ = apply_operation_ticks(
        world,
        params,
        building_catalog,
        building_id,
        worker_id,
        ticks,
    )
    .unwrap();
}

#[test]
fn mine_iron_produces_ore_through_generic_runtime() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) = setup_iron_mine();
    let output = binding_inventory(&world, building_id, "primary_output");
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    complete_one_cycle(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    );
    assert_eq!(count_item(&world, output, "iron_ore"), 1);
}

#[test]
fn smelt_iron_consumes_inputs_and_produces_outputs() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) = setup_smelter();
    let ore = binding_inventory(&world, building_id, "ore_input");
    let metal = binding_inventory(&world, building_id, "metal_output");
    let slag = binding_inventory(&world, building_id, "slag_output");
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    complete_one_cycle(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    );
    assert_eq!(count_item(&world, ore, "iron_ore"), 8);
    assert_eq!(count_item(&world, metal, "iron_bar"), 1);
    assert_eq!(count_item(&world, slag, "slag"), 1);
}

#[test]
fn missing_input_blocks_without_consuming() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) = setup_smelter();
    let ore = binding_inventory(&world, building_id, "ore_input");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let _ = crate::world::consume_stack_item(
        inventory_store,
        instance_store,
        test_inventory_ctx(),
        ore,
        &ItemDefinitionId::new("iron_ore"),
        9,
    );
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    world
        .building_production_store_mut()
        .get_state_mut(building_id)
        .progress = ProductionProgress(PRODUCTION_PROGRESS_ONE_UNIT);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert_eq!(report.lifecycle, OperationLifecycle::Blocked);
    assert_eq!(report.limiting_factor, OperationalLimitingFactor::MissingInput);
    assert_eq!(count_item(&world, ore, "iron_ore"), 1);
}

#[test]
fn output_full_blocks_without_consuming_inputs() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) = setup_smelter();
    let ore = binding_inventory(&world, building_id, "ore_input");
    let metal = binding_inventory(&world, building_id, "metal_output");
    let slag = binding_inventory(&world, building_id, "slag_output");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    for _ in 0..8 {
        let _ = place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            metal,
            ItemDefinitionId::new("iron_bar"),
            50,
        );
        let _ = place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            slag,
            ItemDefinitionId::new("slag"),
            50,
        );
    }
    let ore_before = count_item(&world, ore, "iron_ore");
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    world
        .building_production_store_mut()
        .get_state_mut(building_id)
        .progress = ProductionProgress(PRODUCTION_PROGRESS_ONE_UNIT);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert_eq!(report.lifecycle, OperationLifecycle::Blocked);
    assert_eq!(report.limiting_factor, OperationalLimitingFactor::OutputBlocked);
    assert_eq!(count_item(&world, ore, "iron_ore"), ore_before);
}

#[test]
fn execute_production_cycle_is_atomic_on_output_failure() {
    let (world, building_id, _, _, building_catalog, ops) = setup_smelter();
    let mut world = world;
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .unwrap()
        .clone();
    let op = ops.get(&OperationDefinitionId::new("smelt_iron")).unwrap().clone();
    let metal = binding_inventory(&world, building_id, "metal_output");
    let slag = binding_inventory(&world, building_id, "slag_output");
    let ore_before = count_item(&world, binding_inventory(&world, building_id, "ore_input"), "iron_ore");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    for _ in 0..8 {
        let _ = place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            metal,
            ItemDefinitionId::new("iron_bar"),
            50,
        );
        let _ = place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            slag,
            ItemDefinitionId::new("slag"),
            50,
        );
    }
    let result = execute_production_cycle(
        &mut world,
        test_inventory_ctx(),
        building_id,
        &op,
        &definition,
    );
    assert_eq!(result, Err(OperationalLimitingFactor::OutputBlocked));
    assert_eq!(
        count_item(&world, binding_inventory(&world, building_id, "ore_input"), "iron_ore"),
        ore_before
    );
}

#[test]
fn continuous_mode_executes_multiple_cycles() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) = setup_smelter();
    let metal = binding_inventory(&world, building_id, "metal_output");
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let ticks = crate::world::building::operation::expected_ticks_to_complete(
        EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT,
    ) as u32
        * 3;
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
        ticks,
    )
    .unwrap();
    assert!(count_item(&world, metal, "iron_bar") >= 3);
}

#[test]
fn repeat_count_limits_executed_production() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) = setup_smelter();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .unwrap()
        .clone();
    world
        .building_production_store_mut()
        .ensure_policy_for_building(building_id, &definition, &ops);
    set_production_repeat_count(&mut world, building_id, 2).unwrap();
    let metal = binding_inventory(&world, building_id, "metal_output");
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let ticks = crate::world::building::operation::expected_ticks_to_complete(
        EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT,
    ) as u32
        * 4;
    let _ = apply_operation_ticks(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
        ticks,
    )
    .unwrap();
    let state = world.building_production_store().get_state(building_id).unwrap();
    assert_eq!(state.lifecycle, OperationLifecycle::Completed);
    assert_eq!(state.completion_count, 2);
    assert_eq!(count_item(&world, metal, "iron_bar"), 2);
}

#[test]
fn execution_survives_save_load_without_duplicate_production() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) = setup_smelter();
    let metal = binding_inventory(&world, building_id, "metal_output");
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    complete_one_cycle(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    );
    let bars_after_first = count_item(&world, metal, "iron_bar");
    let exported = world.building_production_store().export_save_state();
    let mut restored = WorldData::new(crate::world::WorldConfig::default().chunk_layout());
    restored
        .building_production_store_mut()
        .import_save_state(exported);
    assert_eq!(
        restored.building_production_store().get_state(building_id),
        world.building_production_store().get_state(building_id)
    );
    assert_eq!(bars_after_first, 1);
}

#[test]
fn assess_production_execution_reports_missing_inputs() {
    let (mut world, building_id, _, catalogs, building_catalog, ops) = setup_smelter();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .unwrap()
        .clone();
    let op = ops.get(&OperationDefinitionId::new("smelt_iron")).unwrap();
    let ore = binding_inventory(&world, building_id, "ore_input");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let _ = crate::world::consume_stack_item(
        inventory_store,
        instance_store,
        test_inventory_ctx(),
        ore,
        &ItemDefinitionId::new("iron_ore"),
        10,
    );
    let assessment = assess_production_execution(
        &world,
        test_inventory_ctx(),
        building_id,
        op,
        &definition,
    );
    assert!(assessment.blocking.is_some());
    assert!(assessment.inputs.iter().any(|input| input.available < input.required));
}

#[test]
fn stack_limit_splits_large_output_quantities() {
    let (mut world, building_id, _, _, building_catalog, ops) = setup_smelter();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .unwrap()
        .clone();
    let mut op = ops.get(&OperationDefinitionId::new("smelt_iron")).unwrap().clone();
    op.outputs = vec![crate::world::OperationOutputDefinition::Item {
        item_id: ItemDefinitionId::new("iron_bar"),
        quantity: 75,
        destination_binding: Some(BuildingInventoryBindingId::new("metal_output")),
    }];
    let ore = binding_inventory(&world, building_id, "ore_input");
    let output = binding_inventory(&world, building_id, "metal_output");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    for _ in 0..4 {
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            ore,
            ItemDefinitionId::new("iron_ore"),
            50,
        )
        .unwrap();
    }
    execute_production_cycle(
        &mut world,
        test_inventory_ctx(),
        building_id,
        &op,
        &definition,
    )
    .unwrap();
    assert_eq!(count_item(&world, output, "iron_bar"), 75);
    let record = world.inventory_store().get(output).unwrap();
    assert!(record.placed_entries().len() > 1);
    assert!(record
        .placed_entries()
        .iter()
        .all(|entry| matches!(
            entry.contents,
            InventoryEntryContents::Stack { quantity, .. } if quantity <= 50
        )));
}
