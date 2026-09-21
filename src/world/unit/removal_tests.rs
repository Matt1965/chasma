//! Unit removal inventory integrity regressions (Slice 1.2).

use bevy::prelude::Vec3;

use super::death::{RemovalReason, UnitRemovalError, step_unit_death_pipeline};
use super::removal::finalize_unit_removal;
use crate::world::equipment::EquipmentSlot;
use crate::world::unit::{UnitDefinitionId, UnitSource};
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, CorpseSettings, Heightfield, InventoryCatalogCtx,
    InventoryError, InventoryOwnerRef, InventoryProfileCatalog, InventoryProfileId, ItemCatalog,
    ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId, ItemDefinition, ItemDefinitionId,
    ItemInstanceMetadata, LocalPosition, TransferPlacementPolicy, UnitCatalog, WorldData,
    WorldPosition, create_item_instance, create_unit_with_inventory, place_unique,
    place_unique_first_fit, starter_inventory_profile_definitions, starter_unit_definitions,
    transfer_unique_item, validate_world_inventory_state,
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

fn test_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories = Box::leak(Box::new(
            ItemCategoryCatalog::from_definitions(vec![
                ItemCategoryDefinition::new(ItemCategoryId::new("armor"), "Armor", "", true),
                ItemCategoryDefinition::new(ItemCategoryId::new("weapon"), "Weapon", "", true),
                ItemCategoryDefinition::new(
                    ItemCategoryId::new("container"),
                    "Container",
                    "",
                    true,
                ),
                ItemCategoryDefinition::new(ItemCategoryId::new("raw_material"), "Raw", "", true),
            ])
            .unwrap(),
        ));
        let items = Box::leak(Box::new(
            ItemCatalog::from_definitions(
                vec![
                    ItemDefinition::new(
                        ItemDefinitionId::new("test_sword"),
                        "Test Sword",
                        "",
                        ItemCategoryId::new("weapon"),
                        1,
                        3,
                        false,
                        1,
                        2_000,
                        1,
                        true,
                    )
                    .with_unique_instance_required(true)
                    .with_equipment_slots(vec![EquipmentSlot::Weapon])
                    .with_weapon_definition_id(
                        crate::world::WeaponDefinitionId::new("weapon_test_sword"),
                    ),
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
                    ItemDefinition::new(
                        ItemDefinitionId::new("test_small_item"),
                        "Small Item",
                        "",
                        ItemCategoryId::new("raw_material"),
                        1,
                        1,
                        false,
                        1,
                        100,
                        1,
                        true,
                    )
                    .with_unique_instance_required(true),
                ],
                categories,
            )
            .unwrap(),
        ));
        let profiles = Box::leak(Box::new(
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap(),
        ));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn spawn_bandit(world: &mut WorldData, ctx: &InventoryCatalogCtx<'_>) -> crate::world::UnitRecord {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    create_unit_with_inventory(
        &catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        crate::world::UnitOwnership::hostile(),
        ctx,
    )
    .unwrap()
}

fn create_unique(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    item_id: &str,
) -> crate::world::ItemInstanceId {
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    create_item_instance(
        inventory_store,
        instance_store,
        ctx,
        ItemDefinitionId::new(item_id),
        ItemInstanceMetadata::default(),
    )
    .unwrap()
}

fn finalize_dev_delete(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit_id: crate::world::UnitId,
) -> Result<(), UnitRemovalError> {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    finalize_unit_removal(
        world,
        unit_id,
        RemovalReason::DevDeleted,
        &catalog,
        ctx,
        &CorpseSettings::default(),
        0,
    )
    .map(|_| ())
}

fn equipment_inventory_ids(
    world: &WorldData,
    unit: &crate::world::UnitRecord,
) -> Vec<crate::world::InventoryId> {
    unit.equipment
        .as_ref()
        .map(|equipment| equipment.all_inventory_ids().to_vec())
        .unwrap_or_default()
}

#[test]
fn dev_delete_empty_equipment_leaves_no_orphan_inventories() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let equipment_ids = equipment_inventory_ids(&world, &unit);
    finalize_dev_delete(&mut world, ctx, unit.id).unwrap();
    assert!(world.get_unit(unit.id).is_none());
    for inventory_id in equipment_ids {
        assert!(world.inventory_store().get(inventory_id).is_none());
    }
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn dev_delete_equipped_item_destroys_instance_and_clears_equipment() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword_id = create_unique(&mut world, ctx, "test_sword");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, sword_id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            sword_id,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    let equipment_ids = equipment_inventory_ids(&world, &unit);
    finalize_dev_delete(&mut world, ctx, unit.id).unwrap();
    assert!(world.get_unit(unit.id).is_none());
    assert!(world.item_instance_store().get(sword_id).is_none());
    for inventory_id in equipment_ids {
        assert!(world.inventory_store().get(inventory_id).is_none());
    }
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn dev_delete_empty_equipped_backpack_clears_container_links() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let backpack_id = create_unique(&mut world, ctx, "test_backpack");
    let internal = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, backpack_id)
            .unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            backpack_id,
            backpack_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    finalize_dev_delete(&mut world, ctx, unit.id).unwrap();
    assert!(world.get_unit(unit.id).is_none());
    assert!(world.item_instance_store().get(backpack_id).is_none());
    assert!(world.inventory_store().get(internal).is_none());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn dev_delete_loaded_equipped_backpack_aborts_without_orphans() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let backpack_id = create_unique(&mut world, ctx, "test_backpack");
    let internal = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let filler = create_unique(&mut world, ctx, "test_small_item");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, backpack_id)
            .unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            backpack_id,
            backpack_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        place_unique(inventory_store, instance_store, ctx, internal, filler, 0, 0).unwrap();
    }
    let equipment_ids = equipment_inventory_ids(&world, &unit);
    let err = finalize_dev_delete(&mut world, ctx, unit.id).unwrap_err();
    assert!(matches!(
        err,
        UnitRemovalError::InventoryCleanupFailed {
            error: InventoryError::ContainerNotEmptyOnRelease { .. },
            ..
        }
    ));
    assert!(world.get_unit(unit.id).is_some());
    for inventory_id in equipment_ids {
        let inventory = world.inventory_store().get(inventory_id).unwrap();
        assert!(matches!(
            inventory.owner(),
            InventoryOwnerRef::UnitEquipment { unit_id, .. } if *unit_id == unit.id
        ));
    }
    assert!(world.item_instance_store().get(backpack_id).is_some());
    assert!(world.inventory_store().get(internal).is_some());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn dev_delete_personal_inventory_contents_destroyed() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let filler = create_unique(&mut world, ctx, "test_small_item");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, filler).unwrap();
    }
    finalize_dev_delete(&mut world, ctx, unit.id).unwrap();
    assert!(world.inventory_store().get(personal).is_none());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn killed_empty_equipment_transitions_to_corpse_and_passes_validation() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = spawn_bandit(&mut world, ctx);
    let equipment_ids = equipment_inventory_ids(&world, &unit);
    world.damage_unit(unit.id, 999).unwrap();
    let report = step_unit_death_pipeline(
        &mut world,
        &catalog,
        Some(ctx),
        &CorpseSettings::default(),
        1,
    );
    assert!(report.removed_unit_ids.contains(&unit.id));
    assert!(world.get_unit(unit.id).is_none());
    let corpse_id = report.corpse_ids[0];
    let corpse = world.corpse_store().get(corpse_id).unwrap();
    assert!(corpse.equipment.is_some());
    for inventory_id in equipment_ids {
        let inventory = world.inventory_store().get(inventory_id).unwrap();
        assert!(matches!(
            inventory.owner(),
            InventoryOwnerRef::CorpseEquipment {
                corpse_id: owner,
                ..
            } if owner == &corpse_id
        ));
    }
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn killed_equipped_weapon_transitions_same_instance_to_corpse_slot() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = spawn_bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword_id = create_unique(&mut world, ctx, "test_sword");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, sword_id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            sword_id,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    world.damage_unit(unit.id, 999).unwrap();
    let report = step_unit_death_pipeline(
        &mut world,
        &catalog,
        Some(ctx),
        &CorpseSettings::default(),
        1,
    );
    assert!(report.removed_unit_ids.contains(&unit.id));
    let corpse_id = report.corpse_ids[0];
    let corpse_weapon = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .equipment
        .unwrap()
        .weapon;
    let weapon_inventory = world.inventory_store().get(corpse_weapon).unwrap();
    assert!(matches!(
        weapon_inventory.owner(),
        InventoryOwnerRef::CorpseEquipment {
            corpse_id: owner,
            slot: EquipmentSlot::Weapon,
        } if *owner == corpse_id
    ));
    assert!(world.item_instance_store().get(sword_id).is_some());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}
