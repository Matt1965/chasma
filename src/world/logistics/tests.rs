//! EP7 generic hauling and logistics runtime tests.

use bevy::prelude::{Quat, Vec3};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operation::{assess_production_execution, execute_production_cycle};
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item, place_stack_first_fit};
use crate::world::logistics::{
    HaulingGenerationReason, HaulingRequestPriority, HaulingRequestStatus, HaulingReservationState,
    LogisticsRouteTrigger, assign_hauling_task, cancel_hauling_request,
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
        let categories = crate::world::ItemCategoryCatalog::from_definitions(
            starter_item_category_definitions(),
        )
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
            .resolve_inventory(building_id, &BuildingInventoryBindingId::new(binding))
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
    assert_eq!(
        request.generation_reason,
        HaulingGenerationReason::OutputSurplus
    );
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
    assert_eq!(
        request.generation_reason,
        HaulingGenerationReason::InputDeficit
    );
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
    crate::world::reserve_hauling_request(&mut fixture.world, request_id, 3, test_inventory_ctx())
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
    crate::world::reserve_hauling_request(&mut fixture.world, request_id, 2, test_inventory_ctx())
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
                .insert_building(crate::world::ChunkId::new(ChunkCoord::new(0, 0)), record)
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
    assert_eq!(
        request.generation_reason,
        HaulingGenerationReason::OutputSurplus
    );
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
fn farm_generates_prispod_output_haul_request() {
    let mut world = flat_world();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
    let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);
    let ctx = test_inventory_ctx();
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
    let farm = create_building_with_inventory(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(70.0, 70.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        ownership,
        None,
        ctx,
    )
    .unwrap();
    let farm_output = world
        .building_inventory_binding_store()
        .resolve_inventory(farm.id, &BuildingInventoryBindingId::new("primary_output"))
        .expect("farm output");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        farm_output,
        ItemDefinitionId::new("prispod"),
        5,
    )
    .unwrap();
    sync_output_surplus_after_production(
        &mut world,
        &building_catalog,
        farm.id,
        &ItemDefinitionId::new("prispod"),
        0,
        ctx,
    );
    let requests: Vec<_> = world
        .hauling_request_store()
        .requests_for_building(farm.id)
        .to_vec();
    assert_eq!(requests.len(), 1);
    let request = world.hauling_request_store().get(requests[0]).unwrap();
    assert_eq!(request.item_id.as_str(), "prispod");
    assert_eq!(
        request.destination_inventory_id,
        world
            .building_inventory_binding_store()
            .resolve_inventory(chest.id, &BuildingInventoryBindingId::new("primary"))
            .expect("chest inventory")
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

mod storage_autonomous {
    use super::*;
    use crate::world::apply_player_storage_accept_all;
    use crate::world::apply_player_storage_category_accepted;
    use crate::world::apply_player_storage_clear_all;
    use crate::world::logistics::HaulingGenerationReason;
    use crate::world::settlement::{
        SettlementKind, SettlementOwnership, assign_building_settlement, assign_unit_settlement,
        create_settlement,
    };
    use crate::world::{BuildingId, ItemCategoryId, assign_hauling_task, reserve_hauling_request};

    fn settlement_fixture() -> (LogisticsFixture, crate::world::SettlementId, BuildingId) {
        let mut fixture = LogisticsFixture::new();
        let settlement = create_settlement(
            &mut fixture.world,
            pos(64.0, 64.0),
            "Haul Town",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            None,
            None,
            0,
        )
        .unwrap();
        let farm = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(60.0, 60.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap();
        fixture.world.mutate_building(farm.id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(
            &mut fixture.world,
            fixture.chest_id,
            Some(settlement.settlement_id),
        )
        .unwrap();
        assign_building_settlement(&mut fixture.world, farm.id, Some(settlement.settlement_id))
            .unwrap();
        (fixture, settlement.settlement_id, farm.id)
    }

    #[test]
    fn generic_storage_generates_prispod_haul_without_explicit_route_resolution_failure() {
        let (mut fixture, _, farm_id) = settlement_fixture();
        let farm_output = fixture.binding_inventory(farm_id, "primary_output");
        fixture.stock_inventory(farm_output, "prispod", 2);
        sync_output_surplus_after_production(
            &mut fixture.world,
            &fixture.building_catalog,
            farm_id,
            &ItemDefinitionId::new("prispod"),
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
        assert_eq!(request.item_id.as_str(), "prispod");
        assert_eq!(
            request.destination_inventory_id,
            fixture.binding_inventory(fixture.chest_id, "primary")
        );
    }

    #[test]
    fn chest_rejecting_food_receives_no_prispod_haul() {
        let (mut fixture, _, farm_id) = settlement_fixture();
        apply_player_storage_category_accepted(
            &mut fixture.world,
            &fixture.building_catalog,
            fixture.chest_id,
            ItemCategoryId::new("food"),
            false,
        )
        .unwrap();
        let farm_output = fixture.binding_inventory(farm_id, "primary_output");
        fixture.stock_inventory(farm_output, "prispod", 2);
        sync_output_surplus_after_production(
            &mut fixture.world,
            &fixture.building_catalog,
            farm_id,
            &ItemDefinitionId::new("prispod"),
            0,
            test_inventory_ctx(),
        );
        assert!(fixture.open_requests().is_empty());
    }

    #[test]
    fn complementary_chest_filters_route_prispod_to_accepting_chest() {
        let (mut fixture, settlement_id, farm_id) = settlement_fixture();
        let rejecting_chest = fixture.chest_id;
        let accepting_chest = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(80.0, 80.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap();
        fixture.world.mutate_building(accepting_chest.id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, accepting_chest.id, Some(settlement_id))
            .unwrap();
        apply_player_storage_category_accepted(
            &mut fixture.world,
            &fixture.building_catalog,
            rejecting_chest,
            ItemCategoryId::new("food"),
            false,
        )
        .unwrap();
        let farm_output = fixture.binding_inventory(farm_id, "primary_output");
        fixture.stock_inventory(farm_output, "prispod", 1);
        sync_output_surplus_after_production(
            &mut fixture.world,
            &fixture.building_catalog,
            farm_id,
            &ItemDefinitionId::new("prispod"),
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
        assert_eq!(
            request.destination_inventory_id,
            fixture.binding_inventory(accepting_chest.id, "primary")
        );
    }

    #[test]
    fn accepted_items_in_accepting_chests_do_not_relocate() {
        let (mut fixture, settlement_id, _) = settlement_fixture();
        let chest_a = fixture.chest_id;
        let chest_b = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(72.0, 72.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        fixture.world.mutate_building(chest_b, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, chest_b, Some(settlement_id)).unwrap();
        fixture.stock_inventory(fixture.binding_inventory(chest_a, "primary"), "prispod", 2);
        crate::world::sync_misfiled_storage_for_building(
            &mut fixture.world,
            &fixture.building_catalog,
            chest_a,
            0,
            test_inventory_ctx(),
        );
        assert!(fixture.open_requests().is_empty());
    }

    #[test]
    fn misfiled_item_generates_relocation_to_accepting_chest() {
        let (mut fixture, settlement_id, _) = settlement_fixture();
        let sort_chest = fixture.chest_id;
        let accepting_chest = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(72.0, 72.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        fixture.world.mutate_building(accepting_chest, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, accepting_chest, Some(settlement_id))
            .unwrap();
        apply_player_storage_clear_all(
            &mut fixture.world,
            &fixture.building_catalog,
            test_inventory_ctx().categories,
            sort_chest,
        )
        .unwrap();
        fixture.stock_inventory(
            fixture.binding_inventory(sort_chest, "primary"),
            "prispod",
            1,
        );
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
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
        assert_eq!(
            request.generation_reason,
            HaulingGenerationReason::MisfiledRelocation
        );
        assert_eq!(
            request.destination_inventory_id,
            fixture.binding_inventory(accepting_chest, "primary")
        );
    }

    #[test]
    fn no_accepting_destination_leaves_misfiled_item_in_place() {
        let (mut fixture, _, _) = settlement_fixture();
        let sort_chest = fixture.chest_id;
        apply_player_storage_clear_all(
            &mut fixture.world,
            &fixture.building_catalog,
            test_inventory_ctx().categories,
            sort_chest,
        )
        .unwrap();
        fixture.stock_inventory(
            fixture.binding_inventory(sort_chest, "primary"),
            "prispod",
            1,
        );
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            0,
            test_inventory_ctx(),
        );
        assert!(fixture.open_requests().is_empty());
        assert_eq!(
            fixture.count_item(fixture.binding_inventory(sort_chest, "primary"), "prispod"),
            1
        );
    }

    #[test]
    fn new_accepting_chest_wakes_relocation() {
        let (mut fixture, settlement_id, _) = settlement_fixture();
        let sort_chest = fixture.chest_id;
        apply_player_storage_clear_all(
            &mut fixture.world,
            &fixture.building_catalog,
            test_inventory_ctx().categories,
            sort_chest,
        )
        .unwrap();
        fixture.stock_inventory(
            fixture.binding_inventory(sort_chest, "primary"),
            "prispod",
            1,
        );
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            0,
            test_inventory_ctx(),
        );
        assert!(fixture.open_requests().is_empty());
        let accepting_chest = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(72.0, 72.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        fixture.world.mutate_building(accepting_chest, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, accepting_chest, Some(settlement_id))
            .unwrap();
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            1,
            test_inventory_ctx(),
        );
        assert_eq!(fixture.open_requests().len(), 1);
    }

    #[test]
    fn accept_all_cancels_obsolete_misfiled_relocation() {
        let (mut fixture, settlement_id, _) = settlement_fixture();
        let sort_chest = fixture.chest_id;
        let accepting_chest = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(72.0, 72.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        fixture.world.mutate_building(accepting_chest, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, accepting_chest, Some(settlement_id))
            .unwrap();
        apply_player_storage_clear_all(
            &mut fixture.world,
            &fixture.building_catalog,
            test_inventory_ctx().categories,
            sort_chest,
        )
        .unwrap();
        fixture.stock_inventory(
            fixture.binding_inventory(sort_chest, "primary"),
            "prispod",
            1,
        );
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            0,
            test_inventory_ctx(),
        );
        assert_eq!(fixture.open_requests().len(), 1);
        crate::world::apply_player_storage_accept_all(
            &mut fixture.world,
            &fixture.building_catalog,
            sort_chest,
        )
        .unwrap();
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            1,
            test_inventory_ctx(),
        );
        assert!(fixture.open_requests().is_empty());
    }

    #[test]
    fn policy_change_on_source_enables_relocation_between_accepting_chests() {
        let (mut fixture, settlement_id, _) = settlement_fixture();
        let chest_a = fixture.chest_id;
        let chest_b = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(72.0, 72.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        fixture.world.mutate_building(chest_b, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, chest_b, Some(settlement_id)).unwrap();
        fixture.stock_inventory(fixture.binding_inventory(chest_a, "primary"), "prispod", 1);
        crate::world::sync_misfiled_storage_for_building(
            &mut fixture.world,
            &fixture.building_catalog,
            chest_a,
            0,
            test_inventory_ctx(),
        );
        assert!(fixture.open_requests().is_empty());
        apply_player_storage_category_accepted(
            &mut fixture.world,
            &fixture.building_catalog,
            chest_a,
            ItemCategoryId::new("food"),
            false,
        )
        .unwrap();
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            1,
            test_inventory_ctx(),
        );
        assert_eq!(fixture.open_requests().len(), 1);
        let request = fixture
            .world
            .hauling_request_store()
            .get(fixture.open_requests()[0])
            .unwrap();
        assert_eq!(
            request.destination_inventory_id,
            fixture.binding_inventory(chest_b, "primary")
        );
    }

    #[test]
    fn schedule_level_sorting_chest_relocation() {
        use crate::world::{deposit_haul_cargo, pickup_haul_cargo, reserve_hauling_request};

        let (mut fixture, settlement_id, _) = settlement_fixture();
        let sort_chest = fixture.chest_id;
        let accepting_chest = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(72.0, 72.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        fixture.world.mutate_building(accepting_chest, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, accepting_chest, Some(settlement_id))
            .unwrap();
        apply_player_storage_clear_all(
            &mut fixture.world,
            &fixture.building_catalog,
            test_inventory_ctx().categories,
            sort_chest,
        )
        .unwrap();
        let sort_inventory = fixture.binding_inventory(sort_chest, "primary");
        let accept_inventory = fixture.binding_inventory(accepting_chest, "primary");
        fixture.stock_inventory(sort_inventory, "prispod", 1);
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            0,
            test_inventory_ctx(),
        );
        let request_id = fixture.open_requests()[0];
        let haul_pos = pos(64.0, 64.0);
        fixture.world.mutate_building(sort_chest, |record| {
            record.placement.position = haul_pos;
        });
        fixture.world.mutate_building(accepting_chest, |record| {
            record.placement.position = haul_pos;
        });
        let worker = fixture.co_locate_for_haul_execution();
        let worker_inventory = fixture
            .world
            .get_unit(worker)
            .and_then(|unit| unit.inventory_id)
            .expect("worker inventory");
        reserve_hauling_request(&mut fixture.world, request_id, 1, test_inventory_ctx()).unwrap();
        pickup_haul_cargo(
            &mut fixture.world,
            request_id,
            worker_inventory,
            1,
            test_inventory_ctx(),
        )
        .unwrap();
        deposit_haul_cargo(
            &mut fixture.world,
            request_id,
            worker_inventory,
            1,
            test_inventory_ctx(),
        )
        .unwrap();
        assert_eq!(fixture.count_item(sort_inventory, "prispod"), 0);
        assert_eq!(fixture.count_item(accept_inventory, "prispod"), 1);
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            1,
            test_inventory_ctx(),
        );
        assert!(fixture.open_requests().is_empty());
    }

    #[test]
    fn general_labor_denied_blocks_misfiled_relocation_assignment() {
        use crate::world::task::TaskPriority;
        use crate::world::{
            WorkPermissionDomain, assign_hauling_task_with_priority, set_unit_work_permission,
        };

        let (mut fixture, settlement_id, _) = settlement_fixture();
        let sort_chest = fixture.chest_id;
        let accepting_chest = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(72.0, 72.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        fixture.world.mutate_building(accepting_chest, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, accepting_chest, Some(settlement_id))
            .unwrap();
        apply_player_storage_clear_all(
            &mut fixture.world,
            &fixture.building_catalog,
            test_inventory_ctx().categories,
            sort_chest,
        )
        .unwrap();
        fixture.stock_inventory(
            fixture.binding_inventory(sort_chest, "primary"),
            "prispod",
            1,
        );
        crate::world::sync_dirty_storage_logistics(
            &mut fixture.world,
            &fixture.building_catalog,
            0,
            test_inventory_ctx(),
        );
        let request_id = fixture.open_requests()[0];
        let worker = fixture.co_locate_for_haul_execution();
        assign_unit_settlement(&mut fixture.world, worker, Some(settlement_id)).unwrap();
        set_unit_work_permission(
            &mut fixture.world,
            settlement_id,
            worker,
            WorkPermissionDomain::GeneralLabor,
            false,
        )
        .unwrap();
        let result = assign_hauling_task_with_priority(
            &mut fixture.world,
            &fixture.unit_catalog,
            &WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            test_inventory_ctx(),
            worker,
            request_id,
            TaskPriority::Normal,
            0,
        );
        assert!(result.is_err());
    }

    #[test]
    fn quarry_output_can_use_generic_storage_fallback() {
        let (mut fixture, settlement_id, _) = settlement_fixture();
        let quarry = create_building_with_inventory(
            &fixture.building_catalog,
            &mut fixture.world,
            &BuildingDefinitionId::new("stone_quarry"),
            pos(55.0, 55.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap();
        fixture.world.mutate_building(quarry.id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(&mut fixture.world, quarry.id, Some(settlement_id)).unwrap();
        let quarry_output = fixture.binding_inventory(quarry.id, "primary_output");
        fixture.stock_inventory(quarry_output, "stone", 3);
        sync_output_surplus_after_production(
            &mut fixture.world,
            &fixture.building_catalog,
            quarry.id,
            &ItemDefinitionId::new("stone"),
            0,
            test_inventory_ctx(),
        );
        let requests = fixture.open_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            fixture
                .world
                .hauling_request_store()
                .get(requests[0])
                .unwrap()
                .item_id
                .as_str(),
            "stone"
        );
    }

    #[test]
    fn destination_fit_counts_partial_stack_headroom() {
        let (mut fixture, _, _) = settlement_fixture();
        let chest = fixture.binding_inventory(fixture.chest_id, "primary");
        fixture.stock_inventory(chest, "prispod", 5);
        let reservations = fixture.world.inventory_reservation_store();
        let ctx = test_inventory_ctx();
        let item = ItemDefinitionId::new("prispod");
        assert!(
            crate::world::destination_can_fit_stack_quantity(
                fixture.world.inventory_store(),
                reservations,
                ctx,
                chest,
                &item,
                15,
            ),
            "partial stack should accept merge headroom"
        );
        assert!(
            !crate::world::destination_can_fit_stack_quantity(
                fixture.world.inventory_store(),
                reservations,
                ctx,
                chest,
                &item,
                16,
            ),
            "quantity beyond stack cap must not fit"
        );
    }

    #[test]
    fn destination_fit_matches_merge_transfer_executor() {
        use crate::world::inventory::{TransferPlacementPolicy, transfer_stack_quantity};

        let (mut fixture, _, _) = settlement_fixture();
        let destination = fixture.binding_inventory(fixture.chest_id, "primary");
        let source = fixture.binding_inventory(fixture.mine_id, "primary_output");
        fixture.stock_inventory(destination, "prispod", 5);
        fixture.stock_inventory(source, "prispod", 20);
        let reservations = fixture.world.inventory_reservation_store();
        let ctx = test_inventory_ctx();
        let item = ItemDefinitionId::new("prispod");
        let quantity = 15u32;

        let can_fit = crate::world::destination_can_fit_stack_quantity(
            fixture.world.inventory_store(),
            reservations,
            ctx,
            destination,
            &item,
            quantity,
        );
        assert!(can_fit, "predicate must allow merge headroom transfer");
        let (inventory_store, instance_store) = fixture.world.inventory_runtime_mut();
        let report = transfer_stack_quantity(
            inventory_store,
            instance_store,
            ctx,
            source,
            0,
            destination,
            quantity,
            TransferPlacementPolicy::MergeThenFirstFit,
            false,
        )
        .expect("executor must succeed when predicate passes");
        assert_eq!(report.moved, quantity);
        let record = fixture.world.inventory_store().get(destination).unwrap();
        assert_eq!(
            record.placed_entries().len(),
            1,
            "must merge into one stack"
        );
        assert_eq!(fixture.count_item(destination, "prispod"), 20);
    }
}

mod schedule_proof {
    use super::*;
    use crate::simulation::{SIMULATION_TICK_SECONDS, run_simulation_tick};
    use crate::world::apply_player_storage_clear_all;
    use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
    use crate::world::building::inventory::attach_inventory_on_building_create;
    use crate::world::building::operation::{
        BASE_OPERATION_PROGRESS_PER_TICK, BuildingOperationParams, FarmProductionPhase,
        PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress, expected_ticks_to_complete,
    };
    use crate::world::logistics::types::BLOCKED_HAUL_RETRY_COOLDOWN_TICKS;
    use crate::world::logistics::{HaulingRequestStatus, retry_blocked_hauling_requests};
    use crate::world::settlement::{
        SettlementKind, SettlementOwnership, assign_building_settlement, assign_unit_settlement,
        create_settlement, ensure_settlement_states_for_world,
    };
    use crate::world::task::TaskType;
    use crate::world::{
        AuthoredRelationshipCatalog, BuildingConstructionSettings,
        BuildingInteractionProfileCatalog, BuildingLifecycleState,
        BuildingNavigationBlueprintCatalog, BuildingOwnership, BuildingSource, ChunkCoord,
        ChunkData, ChunkExtent, ChunkId, ChunkLayout, CombatAiScanState, CombatAiSettings,
        CorpseSettings, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, NavigationConfig,
        TerrainFieldId, UnitOwnership, UnitSource, WeaponCatalog, WorkPermissionDomain,
        WorldPosition, bootstrap_constant_field, create_unit_with_inventory,
        field_value_from_percent, seed_building_settlement_at_creation, set_unit_work_permission,
        starter_weapon_definitions,
    };

    fn navigable_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        bootstrap_constant_field(
            world.terrain_fields_mut(),
            TerrainFieldId::new("water"),
            ChunkCoord::new(0, 0),
            field_value_from_percent(50.0),
        );
        world
    }

    fn unit_global_xz(world: &WorldData, unit_id: crate::world::UnitId) -> (f32, f32) {
        let unit = world.get_unit(unit_id).unwrap();
        let global = unit.placement.position.to_global(world.layout());
        (global.x, global.z)
    }

    struct FarmToChestHarness {
        world: WorldData,
        settlement_id: crate::world::SettlementId,
        farm_id: crate::world::BuildingId,
        chest_id: crate::world::BuildingId,
        farmer_id: crate::world::UnitId,
        worker_id: crate::world::UnitId,
        building_catalog: BuildingCatalog,
        operation_catalog: OperationCatalog,
        unit_catalog: UnitCatalog,
        weapons: WeaponCatalog,
        doodad: DoodadCatalog,
        footprint: FootprintCatalog,
        nav: NavigationConfig,
        interaction: BuildingInteractionProfileCatalog,
        interior: InteriorProfileCatalog,
        nav_blueprint: BuildingNavigationBlueprintCatalog,
        combat_scan: CombatAiScanState,
        terrain_catalogs:
            crate::world::building::terrain_assessment::TerrainAssessmentCatalogs<'static>,
        assessment_store: crate::world::BuildingTerrainAssessmentStore,
    }

    impl FarmToChestHarness {
        fn new() -> Self {
            let mut world = navigable_world();
            let settlement_id = create_settlement(
                &mut world,
                pos(64.0, 64.0),
                "Haul Proof Town",
                SettlementOwnership::player_default(),
                SettlementKind::Town,
                None,
                None,
                0,
            )
            .unwrap()
            .settlement_id;
            ensure_settlement_states_for_world(&mut world);
            if let Some(state) = world.settlement_state_store_mut().get_mut(settlement_id) {
                state.policies.automation_enabled = false;
            }

            let categories = BuildingCategoryCatalog::default();
            let building_catalog =
                BuildingCatalog::from_definitions(starter_building_definitions(), &categories)
                    .unwrap();
            let operation_catalog =
                OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
            let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
            let ctx = test_inventory_ctx();
            let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);

            let farm = create_building_with_inventory(
                &building_catalog,
                &mut world,
                &BuildingDefinitionId::new("prispod_farm"),
                pos(58.0, 58.0),
                Quat::IDENTITY,
                BuildingSource::Authored,
                ownership,
                None,
                ctx,
            )
            .unwrap();
            let chest = create_building_with_inventory(
                &building_catalog,
                &mut world,
                &BuildingDefinitionId::new("storage_chest"),
                pos(60.0, 60.0),
                Quat::IDENTITY,
                BuildingSource::Authored,
                ownership,
                None,
                ctx,
            )
            .unwrap();
            for building_id in [farm.id, chest.id] {
                world.mutate_building(building_id, |record| {
                    record.lifecycle_state = BuildingLifecycleState::Complete;
                });
            }
            assign_building_settlement(&mut world, farm.id, Some(settlement_id)).unwrap();
            assign_building_settlement(&mut world, chest.id, Some(settlement_id)).unwrap();
            seed_building_settlement_at_creation(&mut world, farm.id, pos(58.0, 58.0));

            let farm_definition = building_catalog
                .get(&BuildingDefinitionId::new("prispod_farm"))
                .expect("farm");
            {
                let store = world.building_production_store_mut();
                store.ensure_policy_for_building(farm.id, farm_definition, &operation_catalog);
                store.get_policy_mut(farm.id).enabled = true;
                let farm_state = store.farm_state_mut(farm.id);
                farm_state.phase = FarmProductionPhase::Growing;
                farm_state.growth_progress = ProductionProgress(
                    PRODUCTION_PROGRESS_ONE_UNIT - BASE_OPERATION_PROGRESS_PER_TICK,
                );
                farm_state.harvest_progress = ProductionProgress::ZERO;
            }

            let farmer_id = create_unit_with_inventory(
                &unit_catalog,
                &mut world,
                &UnitDefinitionId::new("bandit"),
                pos(55.0, 55.0),
                UnitSource::Authored,
                UnitOwnership::player_default(),
                ctx,
            )
            .unwrap()
            .id;
            assign_unit_settlement(&mut world, farmer_id, Some(settlement_id)).unwrap();

            let worker_id = create_unit_with_inventory(
                &unit_catalog,
                &mut world,
                &UnitDefinitionId::new("bandit"),
                pos(52.0, 52.0),
                UnitSource::Authored,
                UnitOwnership::player_default(),
                ctx,
            )
            .unwrap()
            .id;
            assign_unit_settlement(&mut world, worker_id, Some(settlement_id)).unwrap();
            set_unit_work_permission(
                &mut world,
                settlement_id,
                farmer_id,
                WorkPermissionDomain::GeneralLabor,
                false,
            )
            .unwrap();
            set_unit_work_permission(
                &mut world,
                settlement_id,
                worker_id,
                WorkPermissionDomain::Farming,
                false,
            )
            .unwrap();

            let terrain_catalogs = {
                let buildings = Box::leak(Box::new(building_catalog.clone()));
                crate::world::building::terrain_assessment::TerrainAssessmentCatalogs {
                    buildings,
                    requirements: Box::leak(Box::new(
                        crate::world::BuildingFieldRequirementCatalog::default(),
                    )),
                    profiles: Box::leak(Box::new(
                        crate::world::FieldResponseProfileCatalog::default(),
                    )),
                    fields: Box::leak(Box::new(crate::world::TerrainFieldCatalog::default())),
                    footprints: Box::leak(Box::new(FootprintCatalog::default())),
                    requirement_revision: 0,
                    profile_revision: 0,
                }
            };

            Self {
                world,
                settlement_id,
                farm_id: farm.id,
                chest_id: chest.id,
                farmer_id,
                worker_id,
                building_catalog,
                operation_catalog,
                unit_catalog,
                weapons: WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap(),
                doodad: DoodadCatalog::default(),
                footprint: FootprintCatalog::default(),
                nav: NavigationConfig::default(),
                interaction: BuildingInteractionProfileCatalog::default(),
                interior: InteriorProfileCatalog::default(),
                nav_blueprint: BuildingNavigationBlueprintCatalog::default(),
                combat_scan: CombatAiScanState::default(),
                terrain_catalogs,
                assessment_store: crate::world::BuildingTerrainAssessmentStore::default(),
            }
        }

        fn farm_output(&self) -> crate::world::InventoryId {
            self.world
                .building_inventory_binding_store()
                .resolve_inventory(
                    self.farm_id,
                    &BuildingInventoryBindingId::new("primary_output"),
                )
                .expect("farm output")
        }

        fn chest_inventory(&self) -> crate::world::InventoryId {
            self.world
                .building_inventory_binding_store()
                .resolve_inventory(self.chest_id, &BuildingInventoryBindingId::new("primary"))
                .expect("chest")
        }

        fn worker_inventory(&self) -> crate::world::InventoryId {
            self.world
                .get_unit(self.worker_id)
                .and_then(|unit| unit.inventory_id)
                .expect("worker inventory")
        }

        fn position_hauler_near(&mut self, position: WorldPosition) {
            self.world.mutate_unit(self.worker_id, |unit| {
                unit.placement.position = position;
            });
        }

        fn count_item(&self, inventory_id: crate::world::InventoryId, item: &str) -> u32 {
            self.world
                .inventory_store()
                .get(inventory_id)
                .map(|record| count_stack_item(record, &ItemDefinitionId::new(item)))
                .unwrap_or(0)
        }

        fn run_tick(&mut self, tick: u64) -> crate::simulation::SimulationTickReport {
            let inventory_ctx = test_inventory_ctx();
            let mut operation = BuildingOperationParams {
                field_catalog: self.terrain_catalogs.fields,
                requirement_catalog: self.terrain_catalogs.requirements,
                profile_catalog: self.terrain_catalogs.profiles,
                footprint_catalog: self.terrain_catalogs.footprints,
                operation_catalog: &self.operation_catalog,
                inventory_ctx,
                requirement_revision: self.terrain_catalogs.requirement_revision,
                profile_revision: self.terrain_catalogs.profile_revision,
                assessment_store: &mut self.assessment_store,
                simulation_tick: tick,
            };
            run_simulation_tick(
                &mut self.world,
                &self.unit_catalog,
                &self.weapons,
                &self.doodad,
                &self.building_catalog,
                &self.footprint,
                &self.interaction,
                &self.nav,
                crate::world::AttackTargetingPolicy::default(),
                &AuthoredRelationshipCatalog::default(),
                &CombatAiSettings::default(),
                &mut self.combat_scan,
                BuildingConstructionSettings::default(),
                &self.interior,
                Some(&self.nav_blueprint),
                inventory_ctx.items,
                inventory_ctx.categories,
                inventory_ctx.profiles,
                &CorpseSettings::default(),
                SIMULATION_TICK_SECONDS,
                tick,
                Some(&mut operation),
            )
        }
    }

    #[test]
    fn scheduled_farm_harvest_generates_haul_claimed_on_later_tick() {
        let mut harness = FarmToChestHarness::new();
        harness.position_hauler_near(pos(57.5, 57.5));
        let harvest_ticks =
            expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
        let max_ticks = harvest_ticks + 1200;
        let start_pos = unit_global_xz(&harness.world, harness.worker_id);

        let mut harvested = false;
        let mut haul_request_seen = false;
        let mut haul_claimed = false;
        let mut position_changed = false;
        let mut carried_in_worker = false;
        let mut deposited_to_chest = false;
        let mut haul_completed = false;
        let mut worker_released = false;
        let mut request_survived_two_ticks = false;
        let mut request_created_tick: Option<u64> = None;
        let mut first_request_id: Option<crate::world::HaulingRequestId> = None;
        let mut haul_cancellations_after_claim = 0u32;
        let mut claim_tick: Option<u64> = None;
        let mut haul_entered_execution = false;

        for tick in 1..=max_ticks {
            let report = harness.run_tick(tick as u64);
            if claim_tick.is_some_and(|claimed_at| tick as u64 > claimed_at) {
                haul_cancellations_after_claim += report.hauling.cancellations;
            }
            if report.hauling.pickups > 0 {
                carried_in_worker = true;
                haul_entered_execution = true;
            }
            if report.hauling.deposits > 0 {
                deposited_to_chest = true;
            }
            if report.hauling.completions > 0 {
                haul_completed = true;
            }

            let farm_output_count = harness.count_item(harness.farm_output(), "prispod");
            if farm_output_count >= 1 {
                harvested = true;
                if !haul_completed {
                    harness
                        .world
                        .building_production_store_mut()
                        .get_policy_mut(harness.farm_id)
                        .paused = true;
                }
            }

            let open = harness
                .world
                .hauling_request_store()
                .sorted_request_ids()
                .into_iter()
                .filter(|id| {
                    harness
                        .world
                        .hauling_request_store()
                        .get(*id)
                        .is_some_and(|r| {
                            r.status.is_open()
                                || r.status == HaulingRequestStatus::InProgress
                                || r.status == HaulingRequestStatus::Assigned
                        })
                })
                .collect::<Vec<_>>();
            if !open.is_empty() {
                haul_request_seen = true;
                if request_created_tick.is_none() {
                    request_created_tick = Some(tick as u64);
                    for request_id in &open {
                        if harness
                            .world
                            .hauling_request_store()
                            .get(*request_id)
                            .is_some_and(|r| r.item_id.as_str() == "prispod")
                        {
                            first_request_id = Some(*request_id);
                        }
                    }
                } else if tick as u64 >= request_created_tick.unwrap() + 2 {
                    request_survived_two_ticks = true;
                }
            }

            for request_id in harness.world.hauling_request_store().sorted_request_ids() {
                let Some(request) = harness.world.hauling_request_store().get(request_id) else {
                    continue;
                };
                if request.assigned_unit_id == Some(harness.worker_id) {
                    haul_claimed = true;
                    if claim_tick.is_none() {
                        claim_tick = Some(tick as u64);
                    }
                }
                if request.picked_up_quantity > 0
                    && harness.count_item(harness.chest_inventory(), "prispod") == 0
                {
                    carried_in_worker = true;
                }
            }
            if harness.count_item(harness.worker_inventory(), "prispod") > 0 {
                carried_in_worker = true;
            }

            let (x, z) = unit_global_xz(&harness.world, harness.worker_id);
            if (x - start_pos.0).abs() > 0.5 || (z - start_pos.1).abs() > 0.5 {
                position_changed = true;
            }

            if harness.count_item(harness.worker_inventory(), "prispod") > 0 {
                carried_in_worker = true;
            }
            if harness.count_item(harness.chest_inventory(), "prispod") > 0 {
                deposited_to_chest = true;
            }

            if let Some(claimed_at) = claim_tick {
                if tick as u64 > claimed_at
                    && harness
                        .world
                        .task_store()
                        .unit_task_id(harness.worker_id)
                        .is_some()
                    && report.hauling.cancellations == 0
                {
                    haul_entered_execution = true;
                }
            }

            let completed = harness
                .world
                .hauling_request_store()
                .sorted_request_ids()
                .into_iter()
                .any(|id| {
                    harness
                        .world
                        .hauling_request_store()
                        .get(id)
                        .is_some_and(|r| r.status == HaulingRequestStatus::Completed)
                });
            if completed {
                haul_completed = true;
            }
            if harness
                .world
                .task_store()
                .unit_task_id(harness.worker_id)
                .is_none()
                && harvested
                && haul_completed
                && deposited_to_chest
                && carried_in_worker
            {
                worker_released = true;
                break;
            }
        }

        assert!(harvested, "farm must produce prispod in output");
        assert!(haul_request_seen, "haul request must be generated");
        assert!(
            first_request_id.is_some_and(|id| id.is_valid()),
            "first fresh-world haul request id must be valid (nonzero)"
        );
        assert!(
            request_survived_two_ticks,
            "haul request must survive >=2 ticks after creation"
        );
        assert!(
            haul_claimed,
            "worker must claim haul on a later assignment tick"
        );
        assert_eq!(
            haul_cancellations_after_claim, 0,
            "haul task must not be cancelled after claim during successful path"
        );
        assert!(
            haul_entered_execution,
            "haul task must survive claim and enter execution (pickup or active task)"
        );
        assert!(
            position_changed,
            "worker world position must change during haul"
        );
        assert!(
            carried_in_worker,
            "prispod must pass through worker inventory"
        );
        assert!(
            deposited_to_chest,
            "chest must receive prispod (farm_out={} chest={} worker={})",
            harness.count_item(harness.farm_output(), "prispod"),
            harness.count_item(harness.chest_inventory(), "prispod"),
            harness.count_item(harness.worker_inventory(), "prispod"),
        );
        assert!(haul_completed, "haul request must complete");
        assert!(worker_released, "worker must release after haul completes");
        assert_eq!(harness.count_item(harness.farm_output(), "prispod"), 0);
    }

    #[test]
    fn manual_unit_prispod_is_not_auto_dumped_to_storage() {
        let mut harness = FarmToChestHarness::new();
        let worker_inventory = harness.worker_inventory();
        let (inventory_store, instance_store) = harness.world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            worker_inventory,
            ItemDefinitionId::new("prispod"),
            1,
        )
        .unwrap();

        for tick in 1..=120 {
            harness.run_tick(tick as u64);
        }

        assert_eq!(harness.count_item(worker_inventory, "prispod"), 1);
        let unit_haul_requests = harness
            .world
            .hauling_request_store()
            .sorted_request_ids()
            .into_iter()
            .filter(|id| {
                harness
                    .world
                    .hauling_request_store()
                    .get(*id)
                    .is_some_and(|request| {
                        request.source_inventory_id == worker_inventory
                            || request.destination_inventory_id == worker_inventory
                    })
            })
            .count();
        assert_eq!(unit_haul_requests, 0);
    }

    #[test]
    fn blocked_haul_request_recovers_after_cooldown() {
        let mut harness = FarmToChestHarness::new();
        let farm_output = harness.farm_output();
        let chest = harness.chest_inventory();
        let (inventory_store, instance_store) = harness.world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            farm_output,
            ItemDefinitionId::new("prispod"),
            3,
        )
        .unwrap();
        sync_output_surplus_after_production(
            &mut harness.world,
            &harness.building_catalog,
            harness.farm_id,
            &ItemDefinitionId::new("prispod"),
            1,
            test_inventory_ctx(),
        );
        let request_id = harness
            .world
            .hauling_request_store()
            .sorted_request_ids()
            .into_iter()
            .find(|id| {
                harness
                    .world
                    .hauling_request_store()
                    .get(*id)
                    .is_some_and(|r| r.item_id.as_str() == "prispod")
            })
            .expect("prispod haul request");

        crate::world::logistics::release_request_reservations(
            harness.world.inventory_reservation_store_mut(),
            request_id,
            &ItemDefinitionId::new("prispod"),
        );
        if let Some(request) = harness
            .world
            .hauling_request_store_mut()
            .get_mut(request_id)
        {
            request.status = HaulingRequestStatus::Blocked;
            request.blocking_reason =
                Some(crate::world::logistics::types::HaulingBlockingReason::DestinationFull);
            request.blocked_at_tick = Some(10);
            request.assigned_unit_id = None;
            request.assigned_task_id = None;
            harness
                .world
                .hauling_request_store_mut()
                .refresh_open_key(request_id);
        }

        sync_output_surplus_after_production(
            &mut harness.world,
            &harness.building_catalog,
            harness.farm_id,
            &ItemDefinitionId::new("prispod"),
            20,
            test_inventory_ctx(),
        );
        let request_ids = harness.world.hauling_request_store().sorted_request_ids();
        assert_eq!(
            request_ids,
            vec![request_id],
            "surplus sync must not create a duplicate haul row while blocked"
        );
        assert_eq!(
            harness
                .world
                .hauling_request_store()
                .get(request_id)
                .unwrap()
                .status,
            HaulingRequestStatus::Pending,
            "surplus sync should revive blocked request instead of creating a duplicate row"
        );

        let inventory_ctx = test_inventory_ctx();
        retry_blocked_hauling_requests(
            &mut harness.world,
            &harness.building_catalog,
            10 + BLOCKED_HAUL_RETRY_COOLDOWN_TICKS + 1,
            inventory_ctx,
        );
        let revived = harness
            .world
            .hauling_request_store()
            .get(request_id)
            .unwrap();
        assert_eq!(revived.status, HaulingRequestStatus::Pending);
        assert!(revived.assigned_unit_id.is_none());
        assert!(revived.remaining_quantity >= 3);
    }

    #[test]
    fn partial_haul_respects_worker_carry_capacity() {
        let mut harness = FarmToChestHarness::new();
        harness
            .world
            .building_production_store_mut()
            .get_policy_mut(harness.farm_id)
            .paused = true;
        let farm_output = harness.farm_output();
        let worker_inventory = harness.worker_inventory();
        let surplus_quantity = 20u32;
        let (inventory_store, instance_store) = harness.world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            farm_output,
            ItemDefinitionId::new("prispod"),
            surplus_quantity,
        )
        .unwrap();
        for _ in 0..35 {
            place_stack_first_fit(
                inventory_store,
                instance_store,
                test_inventory_ctx(),
                worker_inventory,
                ItemDefinitionId::new("gold"),
                1,
            )
            .unwrap();
        }
        sync_output_surplus_after_production(
            &mut harness.world,
            &harness.building_catalog,
            harness.farm_id,
            &ItemDefinitionId::new("prispod"),
            0,
            test_inventory_ctx(),
        );
        let request_id = harness
            .world
            .hauling_request_store()
            .sorted_request_ids()
            .into_iter()
            .find(|id| {
                harness
                    .world
                    .hauling_request_store()
                    .get(*id)
                    .is_some_and(|r| r.item_id.as_str() == "prispod")
            })
            .expect("prispod haul");

        for tick in 1..=500 {
            harness.run_tick(tick as u64);
            let request = harness.world.hauling_request_store().get(request_id);
            if let Some(request) = request {
                if request.remaining_quantity < surplus_quantity {
                    assert!(harness.count_item(harness.chest_inventory(), "prispod") > 0);
                    return;
                }
            }
        }
        panic!("expected multi-trip partial haul when worker capacity is limited");
    }

    #[test]
    fn scheduled_misfiled_relocation_physically_moves_prispod() {
        let mut harness = FarmToChestHarness::new();
        harness.position_hauler_near(pos(59.5, 60.5));
        harness
            .world
            .building_production_store_mut()
            .get_policy_mut(harness.farm_id)
            .paused = true;
        {
            let store = harness.world.building_production_store_mut();
            let farm = store.farm_state_mut(harness.farm_id);
            farm.phase = FarmProductionPhase::Growing;
            farm.growth_progress = ProductionProgress::ZERO;
            farm.harvest_progress = ProductionProgress::ZERO;
        }
        apply_player_storage_clear_all(
            &mut harness.world,
            &harness.building_catalog,
            test_inventory_ctx().categories,
            harness.chest_id,
        )
        .unwrap();
        let sort_chest = harness.chest_id;
        let accepting_chest = create_building_with_inventory(
            &harness.building_catalog,
            &mut harness.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(90.0, 58.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        harness.world.mutate_building(accepting_chest, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assign_building_settlement(
            &mut harness.world,
            accepting_chest,
            Some(harness.settlement_id),
        )
        .unwrap();
        let sort_inventory = harness.chest_inventory();
        let accept_inventory = harness
            .world
            .building_inventory_binding_store()
            .resolve_inventory(accepting_chest, &BuildingInventoryBindingId::new("primary"))
            .expect("accepting chest");
        let (inventory_store, instance_store) = harness.world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            sort_inventory,
            ItemDefinitionId::new("prispod"),
            1,
        )
        .unwrap();
        crate::world::sync_dirty_storage_logistics(
            &mut harness.world,
            &harness.building_catalog,
            1,
            test_inventory_ctx(),
        );

        let start_pos = unit_global_xz(&harness.world, harness.worker_id);
        let mut position_changed = false;
        let mut deposited = false;

        for tick in 1..=1200 {
            let report = harness.run_tick(tick as u64);
            if report.hauling.deposits > 0 {
                deposited = true;
            }
            let (x, z) = unit_global_xz(&harness.world, harness.worker_id);
            if (x - start_pos.0).abs() > 0.5 || (z - start_pos.1).abs() > 0.5 {
                position_changed = true;
            }
            if harness.count_item(accept_inventory, "prispod") > 0 {
                deposited = true;
                break;
            }
        }

        assert!(
            position_changed,
            "worker must walk to relocate misfiled prispod"
        );
        assert!(deposited, "accepting chest must receive prispod");
        assert_eq!(harness.count_item(sort_inventory, "prispod"), 0);
    }
}
