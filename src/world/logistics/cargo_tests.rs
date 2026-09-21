//! Backpack-aware hauling integration tests (Slice 5).

use bevy::prelude::{Quat, Vec3};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::equipment::EquipmentSlot;
use crate::world::inventory::{
    InventoryCatalogCtx, max_accept_stack_quantity, place_stack_first_fit,
};
use crate::world::logistics::{
    HaulingRequestPriority, deposit_haul_cargo, pickup_haul_cargo, reserve_hauling_request,
    spawn_manual_hauling_request,
};
use crate::world::unit::{
    NutritionProfile, UnitDefinitionId, UnitSource, eat_one_from_inventory, select_food_source,
};
use crate::world::{
    Affiliation, BuildingDefinitionId, BuildingLifecycleState, BuildingOwnership, BuildingSource,
    ChunkCoord, ChunkExtent, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, LocalPosition,
    TransferPlacementPolicy, UnitCatalog, UnitOwnership, WorldData, WorldPosition,
    create_building_with_inventory, create_item_instance, create_unit_with_inventory,
    place_unique_first_fit, starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
    test_equipment_fixture_definitions, transfer_unique_item, worker_cargo_inventories,
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
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let mut item_defs = starter_item_definitions();
        item_defs.extend(test_equipment_fixture_definitions());
        let items = ItemCatalog::from_definitions(item_defs, &categories).unwrap();
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

struct BackpackHaulFixture {
    world: WorldData,
    building_catalog: BuildingCatalog,
    unit_catalog: UnitCatalog,
    chest_id: crate::world::BuildingId,
    smelter_id: crate::world::BuildingId,
}

impl BackpackHaulFixture {
    fn new() -> Self {
        let mut world = flat_world();
        let categories = crate::world::BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
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
        for building_id in [chest.id, smelter.id] {
            world.mutate_building(building_id, |record| {
                record.lifecycle_state = BuildingLifecycleState::Complete;
            });
        }
        Self {
            world,
            building_catalog,
            unit_catalog,
            chest_id: chest.id,
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
                &crate::world::building::inventory_binding::BuildingInventoryBindingId::new(
                    binding,
                ),
            )
            .expect("binding")
    }

    fn count_item(&self, inventory_id: crate::world::InventoryId, item: &str) -> u32 {
        self.world
            .inventory_store()
            .get(inventory_id)
            .map(|record| {
                crate::world::inventory::count_stack_item(record, &ItemDefinitionId::new(item))
            })
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

    fn spawn_worker(&mut self, position: WorldPosition) -> crate::world::UnitRecord {
        create_unit_with_inventory(
            &self.unit_catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
        &mut self.world,
            &UnitDefinitionId::new("bandit"),
            position,
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            test_inventory_ctx(),
        )
        .unwrap()
    }

    fn equip_leather_backpack(
        &mut self,
        unit: &crate::world::UnitRecord,
    ) -> crate::world::InventoryId {
        let personal = unit.inventory_id.unwrap();
        let backpack_slot = unit.equipment.unwrap().backpack;
        let instance_id = {
            let (inventory_store, instance_store) = self.world.inventory_runtime_mut();
            let id = create_item_instance(
                inventory_store,
                instance_store,
                test_inventory_ctx(),
                ItemDefinitionId::new("leather_backpack"),
                Default::default(),
            )
            .unwrap();
            place_unique_first_fit(
                inventory_store,
                instance_store,
                test_inventory_ctx(),
                personal,
                id,
            )
            .unwrap();
            transfer_unique_item(
                inventory_store,
                instance_store,
                test_inventory_ctx(),
                personal,
                0,
                id,
                backpack_slot,
                TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
            )
            .unwrap();
            id
        };
        self.world
            .item_instance_store()
            .get(instance_id)
            .and_then(|instance| instance.contained_inventory_id)
            .expect("backpack internal")
    }

    fn fill_personal_inventory(&mut self, unit: &crate::world::UnitRecord) {
        let personal = unit.inventory_id.unwrap();
        let ctx = test_inventory_ctx();
        let filler = ItemDefinitionId::new("gold");
        while {
            let (inventory_store, instance_store) = self.world.inventory_runtime_mut();
            place_stack_first_fit(
                inventory_store,
                instance_store,
                ctx,
                personal,
                filler.clone(),
                1,
            )
            .is_ok()
        } {}
    }
}

fn worker_cargo(
    world: &WorldData,
    unit: &crate::world::UnitRecord,
) -> Vec<crate::world::InventoryId> {
    worker_cargo_inventories(world, unit).expect("worker cargo")
}

#[test]
fn pickup_uses_personal_when_room_exists() {
    let mut fixture = BackpackHaulFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 5);
    let worker = fixture.spawn_worker(pos(64.0, 64.0));
    let cargo = worker_cargo(&fixture.world, &worker);
    let personal = worker.inventory_id.unwrap();
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
    reserve_hauling_request(&mut fixture.world, request_id, 3, test_inventory_ctx()).unwrap();
    pickup_haul_cargo(
        &mut fixture.world,
        request_id,
        &cargo,
        3,
        test_inventory_ctx(),
    )
    .unwrap();
    assert_eq!(fixture.count_item(personal, "iron_ore"), 3);
    assert_eq!(cargo.len(), 1);
}

#[test]
fn pickup_overflows_into_equipped_backpack_when_personal_full() {
    let mut fixture = BackpackHaulFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 10);
    let worker = fixture.spawn_worker(pos(64.0, 64.0));
    let internal = fixture.equip_leather_backpack(&worker);
    fixture.fill_personal_inventory(&worker);
    let personal = worker.inventory_id.unwrap();
    let cargo = worker_cargo(&fixture.world, &worker);
    assert_eq!(cargo, vec![personal, internal]);
    let personal_capacity = fixture
        .world
        .inventory_store()
        .get(personal)
        .map(|record| {
            max_accept_stack_quantity(
                record,
                test_inventory_ctx(),
                &ItemDefinitionId::new("iron_ore"),
            )
        })
        .unwrap_or(0);
    assert_eq!(personal_capacity, 0);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        4,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    reserve_hauling_request(&mut fixture.world, request_id, 4, test_inventory_ctx()).unwrap();
    let moved = pickup_haul_cargo(
        &mut fixture.world,
        request_id,
        &cargo,
        4,
        test_inventory_ctx(),
    )
    .unwrap();
    assert!(moved > 0);
    assert_eq!(fixture.count_item(personal, "iron_ore"), 0);
    assert_eq!(fixture.count_item(internal, "iron_ore"), moved);
}

#[test]
fn deposit_from_backpack_only_succeeds() {
    let mut fixture = BackpackHaulFixture::new();
    let source = fixture.binding_inventory(fixture.chest_id, "primary");
    let destination = fixture.binding_inventory(fixture.smelter_id, "ore_input");
    fixture.stock_inventory(source, "iron_ore", 5);
    let worker = fixture.spawn_worker(pos(64.0, 64.0));
    let internal = fixture.equip_leather_backpack(&worker);
    fixture.fill_personal_inventory(&worker);
    let personal = worker.inventory_id.unwrap();
    let cargo = worker_cargo(&fixture.world, &worker);
    let request_id = spawn_manual_hauling_request(
        &mut fixture.world,
        HaulingRequestPriority::Normal,
        ItemDefinitionId::new("iron_ore"),
        2,
        source,
        destination,
        fixture.smelter_id,
        0,
        test_inventory_ctx(),
    )
    .unwrap();
    reserve_hauling_request(&mut fixture.world, request_id, 2, test_inventory_ctx()).unwrap();
    pickup_haul_cargo(
        &mut fixture.world,
        request_id,
        &cargo,
        2,
        test_inventory_ctx(),
    )
    .unwrap();
    assert_eq!(fixture.count_item(internal, "iron_ore"), 2);
    deposit_haul_cargo(
        &mut fixture.world,
        request_id,
        &cargo,
        2,
        test_inventory_ctx(),
    )
    .unwrap();
    assert_eq!(fixture.count_item(internal, "iron_ore"), 0);
    assert_eq!(fixture.count_item(destination, "iron_ore"), 2);
    assert_eq!(fixture.count_item(personal, "iron_ore"), 0);
}

#[test]
fn carried_food_in_backpack_is_selectable_and_edible() {
    let mut fixture = BackpackHaulFixture::new();
    let worker = fixture.spawn_worker(pos(1.0, 1.0));
    let internal = fixture.equip_leather_backpack(&worker);
    fixture.stock_inventory(internal, "prispod", 2);
    let edible = select_food_source(
        &fixture.world,
        &fixture.building_catalog,
        &crate::world::BuildingInteractionProfileCatalog::default(),
        test_inventory_ctx().items,
        worker.id,
        None,
    )
    .expect("edible in backpack");
    assert_eq!(edible.inventory_id, internal);
    let profile = NutritionProfile::from_definition(
        fixture
            .unit_catalog
            .get(&UnitDefinitionId::new("bandit"))
            .unwrap(),
    )
    .unwrap();
    let mut nutrition = fixture.world.get_unit(worker.id).unwrap().nutrition;
    assert!(eat_one_from_inventory(
        &mut fixture.world,
        test_inventory_ctx(),
        worker.id,
        &mut nutrition,
        &profile,
        internal,
        &ItemDefinitionId::new("prispod"),
        test_inventory_ctx().items,
    ));
    assert_eq!(fixture.count_item(internal, "prispod"), 1);
}
