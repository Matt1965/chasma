//! EP7 generic hauling and logistics runtime tests.

use bevy::prelude::{Quat, Vec3};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operation::{
    assess_production_execution, execute_production_cycle,
};
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item, place_stack_first_fit};
use crate::world::logistics::{
    HaulingGenerationReason, HaulingRequestPriority, HaulingRequestStatus,
    HaulingReservationState, LogisticsRouteTrigger, assign_hauling_task, cancel_hauling_request,
    export_logistics_save_state, import_logistics_save_state, spawn_manual_hauling_request,
    step_haul_worker_tasks, sync_logistics_requests_from_assessment,
    sync_output_surplus_after_production,
};
use crate::world::operation::OperationCatalog;
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, DoodadCatalog, ItemDefinitionId,
    LocalPosition, NavigationConfig, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource,
    WeaponCatalog, WorldData, WorldPosition, create_building_with_inventory,
    create_unit_with_inventory, destroy_building, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_operation_definitions, starter_unit_definitions,
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

struct LogisticsFixture {
    world: WorldData,
    building_catalog: BuildingCatalog,
    operation_catalog: OperationCatalog,
    unit_catalog: UnitCatalog,
    chest_id: crate::world::BuildingId,
    mine_id: crate::world::BuildingId,
    smelter_id: crate::world::BuildingId,
}

impl LogisticsFixture {
    fn new() -> Self {
        let mut world = flat_world();
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
        for building_id in [chest.id, mine.id, smelter.id] {
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
        }
    }

    fn binding_inventory(
        &self,
        building_id: crate::world::BuildingId,
        binding: &str,
    ) -> crate::world::InventoryId {
        self.world
            .building_inventory_binding_store()
            .resolve_inventory(
                building_id,
                &BuildingInventoryBindingId::new(binding),
            )
            .expect("binding")
    }

    fn count_item(&self, inventory_id: crate::world::InventoryId, item: &str) -> u32 {
        self.world
            .inventory_store()
            .get(inventory_id)
            .map(|record| count_stack_item(record, &ItemDefinitionId::new(item)))
            .unwrap_or(0)
    }

    fn stock_inventory(
        &mut self,
        inventory_id: crate::world::InventoryId,
        item: &str,
        quantity: u32,
    ) {
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

    fn worker_at(&mut self, position: WorldPosition) -> crate::world::UnitId {
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

    fn co_locate_for_haul_execution(&mut self) -> crate::world::UnitId {
        let haul_pos = pos(64.0, 64.0);
        self.world.mutate_building(self.chest_id, |record| {
            record.placement.position = haul_pos;
        });
        self.world.mutate_building(self.smelter_id, |record| {
            record.placement.position = haul_pos;
        });
        self.worker_at(haul_pos)
    }

    fn smelter_definition(&self) -> &crate::world::BuildingDefinition {
        self.building_catalog
            .get(&BuildingDefinitionId::new("smelter"))
            .expect("smelter definition")
    }

    fn mine_definition(&self) -> &crate::world::BuildingDefinition {
        self.building_catalog
            .get(&BuildingDefinitionId::new("iron_mine"))
            .expect("mine definition")
    }

    fn operation_definition(&self, id: &str) -> &crate::world::OperationDefinition {
        self.operation_catalog
            .get(&crate::world::OperationDefinitionId::new(id))
            .expect("operation")
    }

    fn open_requests(&self) -> Vec<crate::world::HaulingRequestId> {
        self.world
            .hauling_request_store()
            .sorted_request_ids()
            .into_iter()
            .filter(|id| {
                self.world
                    .hauling_request_store()
                    .get(*id)
                    .is_some_and(|request| request.status.is_open())
            })
            .collect()
    }

    fn total_iron_ore(&self) -> u32 {
        let chest = self.binding_inventory(self.chest_id, "primary");
        let mine_out = self.binding_inventory(self.mine_id, "primary_output");
        let smelter_in = self.binding_inventory(self.smelter_id, "ore_input");
        self.count_item(chest, "iron_ore")
            + self.count_item(mine_out, "iron_ore")
            + self.count_item(smelter_in, "iron_ore")
    }
}

#[test]
fn mine_generates_output_haul_request() {
    let mut fixture = LogisticsFixture::new();
    let mine_output = fixture.binding_inventory(fixture.mine_id, "primary_output");
    fixture.stock_inventory(mine_output, "iron_ore", 5);
    sync_output_surplus_after_production(
        &mut fixture.world,
        &fixture.building_catalog,
        fixture.mine_id,
        &ItemDefinitionId::new("iron_ore"),
        0,
        test_inventory_ctx(),
    );
    let requests = fixture.open_requests();
    assert_eq!(requests.len(), 1);
    let request = fixture
        .world
        .hauling_request_store()
        .get(requests[0])
        .unwrap();
    assert_eq!(request.generation_reason, HaulingGenerationReason::OutputSurplus);
    assert_eq!(request.item_id.as_str(), "iron_ore");
    assert_eq!(request.source_inventory_id, mine_output);
    assert_eq!(
        request.destination_inventory_id,
        fixture.binding_inventory(fixture.chest_id, "primary")
    );
}

#[test]
fn smelter_requests_ore_from_storage() {
    let mut fixture = LogisticsFixture::new();
    fixture.stock_inventory(
        fixture.binding_inventory(fixture.chest_id, "primary"),
        "iron_ore",
        10,
    );
    let assessment = assess_production_execution(
        &fixture.world,
        test_inventory_ctx(),
        fixture.smelter_id,
        fixture.operation_definition("smelt_iron"),
        fixture.smelter_definition(),
    );
    sync_logistics_requests_from_assessment(
        &mut fixture.world,
        &fixture.building_catalog,
        fixture.smelter_id,
        &assessment,
        0,
        test_inventory_ctx(),
    );
    let requests = fixture.open_requests();
    assert_eq!(requests.len(), 1);
    let request = fixture
        .world
        .hauling_request_store()
        .get(requests[0])
        .unwrap();
    assert_eq!(request.generation_reason, HaulingGenerationReason::InputDeficit);
    assert_eq!(request.item_id.as_str(), "iron_ore");
    assert_eq!(
        request.source_inventory_id,
        fixture.binding_inventory(fixture.chest_id, "primary")
    );
    assert_eq!(
        request.destination_inventory_id,
        fixture.binding_inventory(fixture.smelter_id, "ore_input")
    );
}

#[test]
fn identical_requests_consolidate_quantities() {
    let mut fixture = LogisticsFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        3,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    );
    spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        4,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    );
    assert_eq!(fixture.open_requests().len(), 1);
    let request = fixture
        .world
        .hauling_request_store()
        .get(fixture.open_requests()[0])
        .unwrap();
    assert_eq!(request.quantity, 7);
    assert_eq!(request.remaining_quantity, 7);
}

#[test]
fn workers_reserve_items_on_assignment() {
    let mut fixture = LogisticsFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 5);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        3,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    let worker = fixture.co_locate_for_haul_execution();
    assign_hauling_task(
        &mut fixture.world,
        &fixture.unit_catalog,
        &WeaponCatalog::default(),
        &DoodadCatalog::default(),
        &NavigationConfig::default(),
        test_inventory_ctx(),
        worker,
        request_id,
        0,
    )
    .unwrap();
    let request = fixture
        .world
        .hauling_request_store()
        .get(request_id)
        .unwrap();
    assert_eq!(
        request.reservation_state,
        HaulingReservationState::FullyReserved
    );
    assert!(
        fixture
            .world
            .inventory_reservation_store()
            .reserved_source_quantity(source, &ItemDefinitionId::new("iron_ore"))
            > 0
    );
}

#[test]
fn workers_transport_items_physically() {
    let mut fixture = LogisticsFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 5);
    let before_total = fixture.total_iron_ore();
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        3,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    let worker = fixture.co_locate_for_haul_execution();
    let worker_inventory = fixture
        .world
        .get_unit(worker)
        .and_then(|unit| unit.inventory_id)
        .expect("worker inventory");
    crate::world::reserve_hauling_request(
        &mut fixture.world,
        request_id,
        3,
        test_inventory_ctx(),
    )
    .unwrap();
    let picked = crate::world::pickup_haul_cargo(
        &mut fixture.world,
        request_id,
        worker_inventory,
        3,
        test_inventory_ctx(),
    )
    .unwrap();
    assert_eq!(picked, 3);
    assert_eq!(fixture.count_item(worker_inventory, "iron_ore"), 3);
    let deposited = crate::world::deposit_haul_cargo(
        &mut fixture.world,
        request_id,
        worker_inventory,
        3,
        test_inventory_ctx(),
    )
    .unwrap();
    assert_eq!(deposited, 3);
    let request = fixture
        .world
        .hauling_request_store()
        .get(request_id)
        .unwrap();
    assert_eq!(request.status, HaulingRequestStatus::Completed);
    assert_eq!(fixture.count_item(source, "iron_ore"), 2);
    assert_eq!(fixture.count_item(destination, "iron_ore"), 3);
    assert_eq!(fixture.count_item(worker_inventory, "iron_ore"), 0);
    assert_eq!(fixture.total_iron_ore(), before_total);
}

#[test]
fn partial_delivery_updates_remaining_quantity() {
    let mut fixture = LogisticsFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 2);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        5,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    let worker = fixture.co_locate_for_haul_execution();
    let worker_inventory = fixture
        .world
        .get_unit(worker)
        .and_then(|unit| unit.inventory_id)
        .expect("worker inventory");
    crate::world::reserve_hauling_request(
        &mut fixture.world,
        request_id,
        2,
        test_inventory_ctx(),
    )
    .unwrap();
    crate::world::pickup_haul_cargo(
        &mut fixture.world,
        request_id,
        worker_inventory,
        2,
        test_inventory_ctx(),
    )
    .unwrap();
    crate::world::deposit_haul_cargo(
        &mut fixture.world,
        request_id,
        worker_inventory,
        2,
        test_inventory_ctx(),
    )
    .unwrap();
    let request = fixture
        .world
        .hauling_request_store()
        .get(request_id)
        .unwrap();
    assert_eq!(request.status, HaulingRequestStatus::PartiallyFulfilled);
    assert_eq!(request.remaining_quantity, 3);
    assert_eq!(fixture.count_item(destination, "iron_ore"), 2);
}

#[test]
fn building_destruction_cancels_owned_requests() {
    let mut fixture = LogisticsFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 5);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        3,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    destroy_building(
        &mut fixture.world,
        &fixture.building_catalog,
        &DoodadCatalog::default(),
        crate::world::OccupancyCatalogs {
            doodad: &DoodadCatalog::default(),
            building: &fixture.building_catalog,
            footprint: &crate::world::FootprintCatalog::default(),
        },
        fixture.smelter_id,
        "test_destroy",
        None,
    )
    .unwrap();
    let request = fixture
        .world
        .hauling_request_store()
        .get(request_id)
        .unwrap();
    assert_eq!(request.status, HaulingRequestStatus::Cancelled);
}

#[test]
fn logistics_survives_save_load_round_trip() {
    let mut fixture = LogisticsFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 4);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::High,
        ItemDefinitionId::new("iron_ore"),
        4,
        source,
        destination,
        fixture.smelter_id,
        7,
        test_inventory_ctx(),
    )
    .unwrap();
    let saved = export_logistics_save_state(&fixture.world);
    let mut restored = flat_world();
    for building_id in [fixture.chest_id, fixture.mine_id, fixture.smelter_id] {
        if let Some(record) = fixture.world.get_building(building_id).cloned() {
            restored
                .insert_building(
                    crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
                    record,
                )
                .unwrap();
        }
    }
    *restored.inventory_store_mut() = fixture.world.inventory_store().clone();
    import_logistics_save_state(&mut restored, saved);
    let request = restored
        .hauling_request_store()
        .get(request_id)
        .expect("request restored");
    assert_eq!(request.quantity, 4);
    assert_eq!(request.remaining_quantity, 4);
    assert_eq!(request.created_tick, 7);
    assert!(
        restored
            .inventory_reservation_store()
            .request_record(request_id)
            .is_some()
            == fixture
                .world
                .inventory_reservation_store()
                .request_record(request_id)
                .is_some()
    );
}

#[test]
fn production_completion_can_trigger_output_haul_route() {
    let mut fixture = LogisticsFixture::new();
    let mine_id = fixture.mine_id;
    let operation_catalog = &fixture.operation_catalog;
    let building_catalog = &fixture.building_catalog;
    crate::world::bootstrap_constant_field(
        fixture.world.terrain_fields_mut(),
        crate::world::TerrainFieldId::new("iron"),
        ChunkCoord::new(0, 0),
        crate::world::field_value_from_percent(100.0),
    );
    let operation = operation_catalog
        .get(&crate::world::OperationDefinitionId::new("mine_iron"))
        .expect("mine_iron");
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("iron_mine"))
        .expect("iron_mine");
    execute_production_cycle(
        &mut fixture.world,
        test_inventory_ctx(),
        mine_id,
        operation,
        definition,
    )
    .unwrap();
    sync_output_surplus_after_production(
        &mut fixture.world,
        building_catalog,
        mine_id,
        &ItemDefinitionId::new("iron_ore"),
        0,
        test_inventory_ctx(),
    );
    assert!(!fixture.open_requests().is_empty());
    let request = fixture
        .world
        .hauling_request_store()
        .get(fixture.open_requests()[0])
        .unwrap();
    assert_eq!(request.generation_reason, HaulingGenerationReason::OutputSurplus);
    assert!(
        building_catalog
            .get(&BuildingDefinitionId::new("iron_mine"))
            .unwrap()
            .logistics_routes
            .iter()
            .any(|route| route.trigger == LogisticsRouteTrigger::OutputSurplus)
    );
}

#[test]
fn cancel_request_releases_reservations() {
    let mut fixture = LogisticsFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 5);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        3,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    let worker = fixture.worker_at(pos(50.0, 50.0));
    assign_hauling_task(
        &mut fixture.world,
        &fixture.unit_catalog,
        &WeaponCatalog::default(),
        &DoodadCatalog::default(),
        &NavigationConfig::default(),
        test_inventory_ctx(),
        worker,
        request_id,
        0,
    )
    .unwrap();
    cancel_hauling_request(&mut fixture.world, request_id);
    assert_eq!(
        fixture
            .world
            .inventory_reservation_store()
            .reserved_source_quantity(source, &ItemDefinitionId::new("iron_ore")),
        0
    );
}
