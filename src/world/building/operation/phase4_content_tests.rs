//! Phase 4 content tests: Prispod food production and stone category (Settlement AI).

use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
use crate::world::building::inventory::attach_inventory_on_building_create;
use crate::world::building::inventory_binding::{
    BuildingInventoryBindingId, effective_inventory_binding_definitions,
};
use crate::world::building::operation::{
    BuildingOperationParams, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    apply_operation_ticks, assess_production_execution, execute_production_cycle,
};
use crate::world::building::terrain_assessment::{
    BuildingTerrainAssessmentStore, TerrainAssessmentCatalogs,
};
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item};
use crate::world::operation::OperationCatalog;
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinition, BuildingId, BuildingLifecycleState,
    BuildingOwnership, BuildingPlacement, BuildingRecord, BuildingSource, ChunkCoord, ChunkExtent,
    ChunkId, FootprintCatalog, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
    ItemCategoryId, ItemDefinitionId, LocalPosition, OperationDefinitionId, TerrainFieldCatalog,
    TerrainFieldId, UnitCatalog, UnitDefinitionId, UnitId, UnitSource, WorldData, WorldPosition,
    bootstrap_constant_field, create_unit, field_value_from_percent, starter_building_definitions,
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

fn operation_catalog() -> OperationCatalog {
    OperationCatalog::from_definitions(starter_operation_definitions()).unwrap()
}

fn prispod_farm_definition() -> BuildingDefinition {
    starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("starter prispod_farm")
}

fn stone_quarry_definition() -> BuildingDefinition {
    starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "stone_quarry")
        .expect("starter stone_quarry")
}

fn terrain_catalogs(
    building_catalog: &crate::world::BuildingCatalog,
) -> TerrainAssessmentCatalogs<'static> {
    let field_catalog = TerrainFieldCatalog::default();
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

fn worker(world: &mut WorldData, position: WorldPosition) -> UnitId {
    let unit_catalog = UnitCatalog::default();
    create_unit(
        &unit_catalog,
        world,
        &UnitDefinitionId::new("wolf"),
        position,
        UnitSource::Authored,
    )
    .unwrap()
    .id
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

fn count_item(world: &WorldData, inventory_id: crate::world::InventoryId, item: &str) -> u32 {
    world
        .inventory_store()
        .get(inventory_id)
        .map(|record| count_stack_item(record, &ItemDefinitionId::new(item)))
        .unwrap_or(0)
}

fn setup_production_building(
    definition: BuildingDefinition,
    field_id: &str,
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
        TerrainFieldId::new(field_id),
        ChunkCoord::new(0, 0),
        field_value_from_percent(field_percent),
    );
    let building_id = world.allocate_building_id();
    let position = pos(64.0, 64.0);
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    place_building(&mut world, &definition, building_id, position);
    let worker_id = worker(&mut world, pos(64.0, 63.0));
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
fn grow_prispods_outputs_prispod_not_iron_ore() {
    let catalog = operation_catalog();
    let op = catalog
        .get(&OperationDefinitionId::new("grow_prispods"))
        .expect("grow_prispods");
    let output = op
        .outputs
        .iter()
        .find_map(|out| {
            if let crate::world::OperationOutputDefinition::Item { item_id, .. } = out {
                Some(item_id.clone())
            } else {
                None
            }
        })
        .expect("item output");
    assert_eq!(output.as_str(), "prispod");
    assert_ne!(output.as_str(), "iron_ore");
}

#[test]
fn prispod_item_is_food_with_positive_nutrition() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let prispod = items
        .get(&ItemDefinitionId::new("prispod"))
        .expect("prispod");
    assert_eq!(prispod.category_id, ItemCategoryId::new("food"));
    assert!(prispod.nutrition > 0);
}

#[test]
fn stone_uses_construction_material_category() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let stone = items.get(&ItemDefinitionId::new("stone")).expect("stone");
    assert_eq!(
        stone.category_id,
        ItemCategoryId::new("construction_material")
    );
    assert_ne!(stone.category_id, ItemCategoryId::new("raw_material"));
}

#[test]
fn non_food_items_default_to_zero_nutrition() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let iron = items
        .get(&ItemDefinitionId::new("iron_ore"))
        .expect("iron_ore");
    assert_eq!(iron.nutrition, 0);
}

#[test]
fn prispod_farm_has_primary_output_binding() {
    let farm = prispod_farm_definition();
    let bindings = effective_inventory_binding_definitions(&farm);
    assert!(
        bindings
            .iter()
            .any(|binding| binding.binding_id.as_str() == "primary_output"),
        "prispod_farm must expose primary_output for grow_prispods"
    );
}

#[test]
fn grow_prispods_places_prispod_in_farm_output_inventory() {
    let (mut world, building_id, _worker, _catalogs, _building_catalog, ops) =
        setup_production_building(prispod_farm_definition(), "water", 50.0);
    let output = binding_inventory(&world, building_id);
    let farm = prispod_farm_definition();
    let op = ops
        .get(&OperationDefinitionId::new("grow_prispods"))
        .expect("grow_prispods");
    let assessment =
        assess_production_execution(&world, test_inventory_ctx(), building_id, op, &farm);
    assert!(
        assessment.blocking.is_none(),
        "expected farm production ready: {:?}",
        assessment
    );
    world
        .building_production_store_mut()
        .get_state_mut(building_id)
        .progress = ProductionProgress(PRODUCTION_PROGRESS_ONE_UNIT);
    execute_production_cycle(&mut world, test_inventory_ctx(), building_id, op, &farm)
        .expect("grow_prispods cycle");
    assert_eq!(count_item(&world, output, "prispod"), 1);
    assert_eq!(count_item(&world, output, "iron_ore"), 0);
}

#[test]
fn grow_prispods_blocks_when_farm_output_buffer_full() {
    use crate::world::inventory::place_stack_first_fit;

    let (mut world, building_id, _worker, _catalogs, _building_catalog, ops) =
        setup_production_building(prispod_farm_definition(), "water", 50.0);
    let output = binding_inventory(&world, building_id);
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        test_inventory_ctx(),
        output,
        ItemDefinitionId::new("prispod"),
        20,
    )
    .expect("fill farm output buffer");
    let farm = prispod_farm_definition();
    let op = ops
        .get(&OperationDefinitionId::new("grow_prispods"))
        .expect("grow_prispods");
    let assessment =
        assess_production_execution(&world, test_inventory_ctx(), building_id, op, &farm);
    assert!(
        assessment.blocking.is_some(),
        "full output buffer should block production: {:?}",
        assessment
    );
}

#[test]
fn prispod_farm_has_output_logistics_route() {
    let farm = prispod_farm_definition();
    assert!(
        farm.logistics_routes
            .iter()
            .any(|route| route.item_id.as_str() == "prispod"),
        "farm should route surplus prispods to storage"
    );
}

#[test]
fn prispod_farm_uses_compact_output_buffer_profile() {
    let farm = prispod_farm_definition();
    let binding = effective_inventory_binding_definitions(&farm)
        .into_iter()
        .find(|binding| binding.binding_id.as_str() == "primary_output")
        .expect("primary_output");
    assert_eq!(binding.profile_id.as_str(), "farm_output_buffer");
}

#[test]
fn placed_prispod_farm_has_no_doors_or_interior_navigation() {
    let (world, building_id, ..) =
        setup_production_building(prispod_farm_definition(), "water", 50.0);
    assert!(
        world.door_store().building_door_ids(building_id).is_empty(),
        "exterior farm must not register doors"
    );
    assert!(
        world
            .building_navigation_runtime()
            .get(building_id)
            .is_none(),
        "farm must not instantiate interior navigation topology"
    );
}

#[test]
fn stone_quarry_still_produces_stone_after_category_change() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) =
        setup_production_building(stone_quarry_definition(), "stone", 80.0);
    let output = binding_inventory(&world, building_id);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    complete_one_cycle(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    );
    assert_eq!(count_item(&world, output, "stone"), 1);
}

#[cfg(feature = "data-import")]
#[test]
fn workbook_prispod_food_and_nutrition_import() {
    use crate::data_import::{dev_design_workbook_path, import_item_catalog_from_excel};

    let path = dev_design_workbook_path();
    let (categories, items, summary) = import_item_catalog_from_excel(&path).unwrap();
    assert_eq!(summary.rows_failed, 0, "{:?}", summary.warnings);
    let prispod = items
        .get(&ItemDefinitionId::new("prispod"))
        .expect("prispod");
    assert_eq!(prispod.category_id, ItemCategoryId::new("food"));
    assert!(prispod.nutrition > 0);
    assert!(
        categories
            .get(&ItemCategoryId::new("construction_material"))
            .is_some()
    );
    let stone = items.get(&ItemDefinitionId::new("stone")).expect("stone");
    assert_eq!(
        stone.category_id,
        ItemCategoryId::new("construction_material")
    );
    let iron = items
        .get(&ItemDefinitionId::new("iron_ore"))
        .expect("iron_ore");
    assert_eq!(iron.nutrition, 0);
}
