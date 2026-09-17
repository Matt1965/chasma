//! Worker cargo inventory resolver tests (Slice 5).

use bevy::prelude::Vec3;

use crate::world::equipment::{
    EquipmentSlot, WorkerCargoResolveError, equipped_backpack_internal_inventory,
    worker_cargo_inventories,
};
use crate::world::unit::{UnitDefinitionId, UnitSource};
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryCatalogCtx,
    InventoryProfileCatalog, InventoryProfileId, ItemCatalog, ItemCategoryCatalog,
    ItemCategoryDefinition, ItemCategoryId, ItemDefinition, ItemDefinitionId, LocalPosition,
    TransferPlacementPolicy, UnitCatalog, UnitOwnership, WorldData, WorldPosition,
    create_item_instance, create_unit_with_inventory, place_unique_first_fit,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_unit_definitions, transfer_unique_item,
};

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
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

fn test_ctx() -> InventoryCatalogCtx<'static> {
    static CATEGORIES: std::sync::OnceLock<ItemCategoryCatalog> = std::sync::OnceLock::new();
    static ITEMS: std::sync::OnceLock<ItemCatalog> = std::sync::OnceLock::new();
    static PROFILES: std::sync::OnceLock<InventoryProfileCatalog> = std::sync::OnceLock::new();

    let categories = CATEGORIES.get_or_init(|| {
        ItemCategoryCatalog::from_definitions(vec![
            ItemCategoryDefinition::new(ItemCategoryId::new("container"), "Container", "", true),
            ItemCategoryDefinition::new(ItemCategoryId::new("raw_material"), "Raw", "", true),
        ])
        .unwrap()
    });
    let items = ITEMS.get_or_init(|| {
        ItemCatalog::from_definitions(
            vec![
                ItemDefinition::new(
                    ItemDefinitionId::new("test_backpack"),
                    "Test Backpack",
                    "",
                    ItemCategoryId::new("container"),
                    2,
                    3,
                    false,
                    1,
                    500,
                    1,
                    true,
                )
                .with_unique_instance_required(true)
                .with_equipment_slots(vec![EquipmentSlot::Backpack])
                .with_backpack_profile_id(InventoryProfileId::new("backpack_basic_internal")),
            ],
            categories,
        )
        .unwrap()
    });
    let profiles = PROFILES.get_or_init(|| {
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap()
    });
    InventoryCatalogCtx::new(items, categories, profiles)
}

fn spawn_worker(world: &mut WorldData, ctx: &InventoryCatalogCtx<'_>) -> crate::world::UnitRecord {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    create_unit_with_inventory(
        &catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::hostile(),
        ctx,
    )
    .unwrap()
}

fn equip_backpack(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit: &crate::world::UnitRecord,
) -> crate::world::InventoryId {
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let instance_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            &ctx,
            ItemDefinitionId::new("test_backpack"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            id,
            backpack_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        id
    };
    world
        .item_instance_store()
        .get(instance_id)
        .and_then(|instance| instance.contained_inventory_id)
        .expect("backpack internal inventory")
}

#[test]
fn worker_without_backpack_returns_personal_only() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_worker(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let cargo = worker_cargo_inventories(&world, &unit).unwrap();
    assert_eq!(cargo, vec![personal]);
}

#[test]
fn worker_with_empty_equipped_backpack_returns_personal_and_internal() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_worker(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let internal = equip_backpack(&mut world, &ctx, &unit);
    let cargo = worker_cargo_inventories(&world, &unit).unwrap();
    assert_eq!(cargo, vec![personal, internal]);
}

#[test]
fn equipment_slot_inventories_are_not_cargo_storage() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_worker(&mut world, &ctx);
    let equipment = unit.equipment.unwrap();
    let cargo = worker_cargo_inventories(&world, &unit).unwrap();
    for slot in EquipmentSlot::ALL {
        assert!(!cargo.contains(&equipment.inventory_id(slot)));
    }
}

#[test]
fn broken_backpack_internal_link_errors() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_worker(&mut world, &ctx);
    let instance_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            &ctx,
            ItemDefinitionId::new("test_backpack"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            unit.inventory_id.unwrap(),
            id,
        )
        .unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            unit.inventory_id.unwrap(),
            0,
            id,
            unit.equipment.unwrap().backpack,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        id
    };
    {
        let instance_store = world.item_instance_store_mut();
        instance_store
            .get_mut(instance_id)
            .expect("instance")
            .contained_inventory_id = None;
    }
    let err = worker_cargo_inventories(&world, &unit).unwrap_err();
    assert!(matches!(
        err,
        WorkerCargoResolveError::MissingBackpackInternalInventoryLink { .. }
    ));
}

#[test]
fn equipped_backpack_internal_absent_when_slot_empty() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_worker(&mut world, &ctx);
    let equipment = unit.equipment.unwrap();
    assert_eq!(
        equipped_backpack_internal_inventory(&world, &equipment).unwrap(),
        None
    );
}
