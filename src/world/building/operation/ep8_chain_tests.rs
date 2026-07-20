//! EP8 multi-building production chain integration tests.

use bevy::prelude::{Quat, Vec3};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operation::{
    assess_production_execution, execute_production_cycle, set_production_repeat_count,
};
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item, place_stack_first_fit};
use crate::world::logistics::{
    HaulingRequestStatus, export_logistics_save_state, import_logistics_save_state,
    reserve_hauling_request, spawn_manual_hauling_request, sync_logistics_requests_from_assessment,
    sync_output_surplus_after_production,
};
use crate::world::operation::OperationCatalog;
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, ItemDefinitionId,
    LocalPosition, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldData,
    WorldPosition, bootstrap_constant_field, create_building_with_inventory,
    create_unit_with_inventory, starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_operation_definitions,
    starter_unit_definitions,
};

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
            crate::world::ItemCategoryCatalog::from_definitions(starter_item_category_definitions())
                .unwrap();
        let items =
            crate::world::ItemCatalog::from_definitions(starter_item_definitions(), &categories)
                .unwrap();
        let profiles = crate::world::InventoryProfileCatalog::from_definitions(
            starter_inventory_profile_definitions(),
        )
        .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

struct ChainFixture {
    world: WorldData,
    building_catalog: BuildingCatalog,
    operation_catalog: OperationCatalog,
    unit_catalog: UnitCatalog,
    chest_id: BuildingId,
    mine_id: BuildingId,
    smelter_id: BuildingId,
    workbench_id: BuildingId,
}

impl ChainFixture {
    fn new() -> Self {
        let mut world = flat_world();
        bootstrap_constant_field(
            world.terrain_fields_mut(),
            crate::world::TerrainFieldId::new("iron"),
            ChunkCoord::new(0, 0),
            crate::world::field_value_from_percent(100.0),
        );
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let operation_catalog =
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let ctx = test_inventory_ctx();
        let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);
        let chest = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap();
        let mine = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("iron_mine"),
            pos(70.0, 70.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap();
        let smelter = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("smelter"),
            pos(90.0, 90.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap();
        let workbench = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("workbench"),
            pos(110.0, 110.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap();
        for building_id in [chest.id, mine.id, smelter.id, workbench.id] {
            world.mutate_building(building_id, |record| {
                record.lifecycle_state = BuildingLifecycleState::Complete;
            });
        }
        Self {
            world,
            building_catalog,
            operation_catalog,
            unit_catalog,
            chest_id: chest.id,
            mine_id: mine.id,
            smelter_id: smelter.id,
            workbench_id: workbench.id,
        }
    }

    fn binding(&self, building_id: BuildingId, binding: &str) -> crate::world::InventoryId {
        self.world
            .building_inventory_binding_store()
            .resolve_inventory(
                building_id,
                &BuildingInventoryBindingId::new(binding),
            )
            .expect("binding")
    }

    fn count(&self, inventory_id: crate::world::InventoryId, item: &str) -> u32 {
        self.world
            .inventory_store()
            .get(inventory_id)
            .map(|record| count_stack_item(record, &ItemDefinitionId::new(item)))
            .unwrap_or(0)
    }

    fn stock(&mut self, inventory_id: crate::world::InventoryId, item: &str, quantity: u32) {
        let (inventory_store, instance_store) = self.world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            ItemDefinitionId::new(item),
            quantity,
        )
        .unwrap();
    }

    fn worker(&mut self, position: WorldPosition) -> crate::world::UnitId {
        create_unit_with_inventory(
            &self.unit_catalog,
            &mut self.world,
            &UnitDefinitionId::new("bandit"),
            position,
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            test_inventory_ctx(),
        )
        .unwrap()
        .id
    }

    fn smelt_op(&self) -> &crate::world::OperationDefinition {
        self.operation_catalog
            .get(&crate::world::OperationDefinitionId::new("smelt_iron"))
            .expect("smelt_iron")
    }

    fn bake_op(&self) -> &crate::world::OperationDefinition {
        self.operation_catalog
            .get(&crate::world::OperationDefinitionId::new("bake_bread"))
            .expect("bake_bread")
    }

    fn smelter_def(&self) -> &crate::world::BuildingDefinition {
        self.building_catalog
            .get(&BuildingDefinitionId::new("smelter"))
            .expect("smelter")
    }

    fn workbench_def(&self) -> &crate::world::BuildingDefinition {
        self.building_catalog
            .get(&BuildingDefinitionId::new("workbench"))
            .expect("workbench")
    }
}

#[test]
fn smelt_iron_uses_generic_runtime_without_fuel() {
    let mut fixture = ChainFixture::new();
    fixture.stock(fixture.binding(fixture.smelter_id, "ore_input"), "iron_ore", 4);
    let smelter_id = fixture.smelter_id;
    let smelt_op = fixture
        .operation_catalog
        .get(&crate::world::OperationDefinitionId::new("smelt_iron"))
        .expect("smelt_iron");
    let smelter_def = fixture
        .building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .expect("smelter");
    execute_production_cycle(
        &mut fixture.world,
        test_inventory_ctx(),
        smelter_id,
        smelt_op,
        smelter_def,
    )
    .unwrap();
    assert_eq!(
        fixture.count(fixture.binding(fixture.smelter_id, "ore_input"), "iron_ore"),
        2
    );
    assert_eq!(
        fixture.count(fixture.binding(fixture.smelter_id, "metal_output"), "iron_bar"),
        1
    );
    assert_eq!(
        fixture.count(fixture.binding(fixture.smelter_id, "slag_output"), "slag"),
        1
    );
}

#[test]
fn bake_bread_requires_all_multi_inputs() {
    let mut fixture = ChainFixture::new();
    fixture.stock(fixture.binding(fixture.workbench_id, "flour_input"), "flour", 2);
    let workbench_id = fixture.workbench_id;
    {
        let bake_op = fixture
            .operation_catalog
            .get(&crate::world::OperationDefinitionId::new("bake_bread"))
            .expect("bake_bread");
        let workbench_def = fixture
            .building_catalog
            .get(&BuildingDefinitionId::new("workbench"))
            .expect("workbench");
        execute_production_cycle(
            &mut fixture.world,
            test_inventory_ctx(),
            workbench_id,
            bake_op,
            workbench_def,
        )
        .expect_err("missing water");
    }
    fixture.stock(fixture.binding(fixture.workbench_id, "water_input"), "water", 1);
    let bake_op = fixture
        .operation_catalog
        .get(&crate::world::OperationDefinitionId::new("bake_bread"))
        .expect("bake_bread");
    let workbench_def = fixture
        .building_catalog
        .get(&BuildingDefinitionId::new("workbench"))
        .expect("workbench");
    execute_production_cycle(
        &mut fixture.world,
        test_inventory_ctx(),
        workbench_id,
        bake_op,
        workbench_def,
    )
    .unwrap();
    assert_eq!(
        fixture.count(fixture.binding(fixture.workbench_id, "bread_output"), "bread"),
        1
    );
}

#[test]
fn smelter_input_deficit_generates_haul_request_from_storage() {
    let mut fixture = ChainFixture::new();
    fixture.stock(fixture.binding(fixture.chest_id, "primary"), "iron_ore", 10);
    let smelter_id = fixture.smelter_id;
    let smelt_op = fixture
        .operation_catalog
        .get(&crate::world::OperationDefinitionId::new("smelt_iron"))
        .expect("smelt_iron");
    let smelter_def = fixture
        .building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .expect("smelter");
    let assessment = assess_production_execution(
        &fixture.world,
        test_inventory_ctx(),
        smelter_id,
        smelt_op,
        smelter_def,
    );
    sync_logistics_requests_from_assessment(
        &mut fixture.world,
        &fixture.building_catalog,
        fixture.smelter_id,
        &assessment,
        0,
        test_inventory_ctx(),
    );
    let requests = fixture
        .world
        .hauling_request_store()
        .sorted_request_ids();
    assert!(!requests.is_empty());
    let request = fixture
        .world
        .hauling_request_store()
        .get(requests[0])
        .unwrap();
    assert_eq!(request.item_id.as_str(), "iron_ore");
    assert_eq!(
        request.source_inventory_id,
        fixture.binding(fixture.chest_id, "primary")
    );
    assert_eq!(
        request.destination_inventory_id,
        fixture.binding(fixture.smelter_id, "ore_input")
    );
}

#[test]
fn production_output_generates_surplus_haul_for_iron_bars() {
    let mut fixture = ChainFixture::new();
    fixture.stock(fixture.binding(fixture.smelter_id, "metal_output"), "iron_bar", 3);
    sync_output_surplus_after_production(
        &mut fixture.world,
        &fixture.building_catalog,
        fixture.smelter_id,
        &ItemDefinitionId::new("iron_bar"),
        0,
        test_inventory_ctx(),
    );
    let request = fixture
        .world
        .hauling_request_store()
        .sorted_request_ids()
        .into_iter()
        .find_map(|id| fixture.world.hauling_request_store().get(id).cloned())
        .expect("haul request");
    assert_eq!(request.item_id.as_str(), "iron_bar");
    assert_eq!(
        request.source_inventory_id,
        fixture.binding(fixture.smelter_id, "metal_output")
    );
}

#[test]
fn reserved_ore_cannot_be_consumed_by_production() {
    let mut fixture = ChainFixture::new();
    let ore_input = fixture.binding(fixture.smelter_id, "ore_input");
    let chest = fixture.binding(fixture.chest_id, "primary");
    fixture.stock(ore_input, "iron_ore", 4);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        crate::world::HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        4,
        ore_input,
        chest,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    reserve_hauling_request(
        &mut fixture.world,
        request_id,
        4,
        test_inventory_ctx(),
    )
    .unwrap();
    let smelter_id = fixture.smelter_id;
    let smelt_op = fixture
        .operation_catalog
        .get(&crate::world::OperationDefinitionId::new("smelt_iron"))
        .expect("smelt_iron");
    let smelter_def = fixture
        .building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .expect("smelter");
    let assessment = assess_production_execution(
        &fixture.world,
        test_inventory_ctx(),
        smelter_id,
        smelt_op,
        smelter_def,
    );
    assert!(matches!(
        assessment.blocking,
        Some(crate::world::building::operation::ProductionExecutionFailure::InputReserved { .. })
    ));
}

#[test]
fn delivered_ore_enables_smelting_after_haul() {
    let mut fixture = ChainFixture::new();
    let chest = fixture.binding(fixture.chest_id, "primary");
    let ore_input = fixture.binding(fixture.smelter_id, "ore_input");
    fixture.stock(chest, "iron_ore", 4);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        crate::world::HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        4,
        chest,
        ore_input,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    let worker = fixture.worker(pos(64.0, 64.0));
    let worker_inventory = fixture
        .world
        .get_unit(worker)
        .and_then(|unit| unit.inventory_id)
        .expect("worker inventory");
    fixture.world.mutate_building(fixture.chest_id, |b| {
        b.placement.position = pos(64.0, 64.0);
    });
    fixture.world.mutate_building(fixture.smelter_id, |b| {
        b.placement.position = pos(64.0, 64.0);
    });
    reserve_hauling_request(&mut fixture.world, request_id, 4, test_inventory_ctx()).unwrap();
    crate::world::pickup_haul_cargo(
        &mut fixture.world,
        request_id,
        worker_inventory,
        4,
        test_inventory_ctx(),
    )
    .unwrap();
    crate::world::deposit_haul_cargo(
        &mut fixture.world,
        request_id,
        worker_inventory,
        4,
        test_inventory_ctx(),
    )
    .unwrap();
    let smelter_id = fixture.smelter_id;
    let smelt_op = fixture.operation_catalog
        .get(&crate::world::OperationDefinitionId::new("smelt_iron"))
        .expect("smelt_iron");
    let smelter_def = fixture.building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .expect("smelter");
    execute_production_cycle(
        &mut fixture.world,
        test_inventory_ctx(),
        smelter_id,
        smelt_op,
        smelter_def,
    )
    .unwrap();
    assert_eq!(fixture.count(ore_input, "iron_ore"), 2);
    assert_eq!(
        fixture.count(fixture.binding(fixture.smelter_id, "metal_output"), "iron_bar"),
        1
    );
}

#[test]
fn repeat_count_stops_after_committed_cycles_only() {
    let mut fixture = ChainFixture::new();
    fixture.stock(fixture.binding(fixture.smelter_id, "ore_input"), "iron_ore", 10);
    set_production_repeat_count(&mut fixture.world, fixture.smelter_id, 2).unwrap();
    let smelter_id = fixture.smelter_id;
    let smelt_op = fixture
        .operation_catalog
        .get(&crate::world::OperationDefinitionId::new("smelt_iron"))
        .expect("smelt_iron");
    let smelter_def = fixture
        .building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .expect("smelter");
    execute_production_cycle(
        &mut fixture.world,
        test_inventory_ctx(),
        smelter_id,
        smelt_op,
        smelter_def,
    )
    .unwrap();
    execute_production_cycle(
        &mut fixture.world,
        test_inventory_ctx(),
        smelter_id,
        smelt_op,
        smelter_def,
    )
    .unwrap();
    assert_eq!(
        fixture.count(fixture.binding(fixture.smelter_id, "metal_output"), "iron_bar"),
        2
    );
    assert_eq!(
        fixture.count(fixture.binding(fixture.smelter_id, "ore_input"), "iron_ore"),
        6
    );
}

#[test]
fn chain_state_survives_logistics_save_load() {
    let mut fixture = ChainFixture::new();
    fixture.stock(fixture.binding(fixture.chest_id, "primary"), "iron_ore", 5);
    let smelter_id = fixture.smelter_id;
    let smelt_op = fixture
        .operation_catalog
        .get(&crate::world::OperationDefinitionId::new("smelt_iron"))
        .expect("smelt_iron");
    let smelter_def = fixture
        .building_catalog
        .get(&BuildingDefinitionId::new("smelter"))
        .expect("smelter");
    let assessment = assess_production_execution(
        &fixture.world,
        test_inventory_ctx(),
        smelter_id,
        smelt_op,
        smelter_def,
    );
    sync_logistics_requests_from_assessment(
        &mut fixture.world,
        &fixture.building_catalog,
        fixture.smelter_id,
        &assessment,
        3,
        test_inventory_ctx(),
    );
    let saved = export_logistics_save_state(&fixture.world);
    let mut restored = flat_world();
    import_logistics_save_state(&mut restored, saved);
    assert_eq!(restored.hauling_request_store().sorted_request_ids().len(), 1);
    let request = restored
        .hauling_request_store()
        .get(restored.hauling_request_store().sorted_request_ids()[0])
        .unwrap();
    assert_eq!(request.created_tick, 3);
    assert_eq!(request.status, HaulingRequestStatus::Pending);
}
