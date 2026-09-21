//! Corpse equipment death, looting, persistence, and expiry regressions (Slice 6).

use bevy::prelude::Vec3;

use super::access::is_corpse_loot_inventory;
use crate::dev::{capture_inventory_persistence, restore_inventory_persistence};
use crate::world::equipment::EquipmentSlot;
use crate::world::inventory::TransferError;
use crate::world::unit::{UnitDefinitionId, UnitSource};
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, CorpseSettings, Heightfield, InventoryCatalogCtx,
    InventoryError, InventoryOwnerRef, InventoryProfileCatalog, InventoryProfileId, ItemCatalog,
    ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId, ItemDefinition, ItemDefinitionId,
    ItemInstanceMetadata, LocalPosition, TransferPlacementPolicy, UnitCatalog, WorldData,
    WorldPosition, create_item_instance, create_unit_with_inventory, place_unique,
    place_unique_first_fit, starter_inventory_profile_definitions, starter_unit_definitions,
    step_corpse_lifecycle, step_unit_death_pipeline, transfer_unique_item,
    validate_world_inventory_state,
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

fn kill_unit(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit_id: crate::world::UnitId,
) -> crate::world::CorpseId {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    world.damage_unit(unit_id, 999).unwrap();
    let report =
        step_unit_death_pipeline(world, &catalog, Some(ctx), &CorpseSettings::default(), 1);
    report.corpse_ids[0]
}

#[test]
fn loaded_backpack_death_preserves_container_and_contents() {
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
    let corpse_id = kill_unit(&mut world, ctx, unit.id);
    let corpse_backpack_slot = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .equipment
        .unwrap()
        .backpack;
    let backpack_inventory = world.inventory_store().get(corpse_backpack_slot).unwrap();
    assert!(matches!(
        backpack_inventory.owner(),
        InventoryOwnerRef::CorpseEquipment {
            corpse_id: owner,
            slot: EquipmentSlot::Backpack,
        } if *owner == corpse_id
    ));
    let instance = world.item_instance_store().get(backpack_id).unwrap();
    assert_eq!(instance.contained_inventory_id, Some(internal));
    assert!(world.inventory_store().get(internal).is_some());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn corpse_loot_access_includes_equipment_and_backpack_internal() {
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
    let corpse_id = kill_unit(&mut world, ctx, unit.id);
    let corpse = world.corpse_store().get(corpse_id).unwrap();
    let equipment = corpse.equipment.unwrap();
    assert!(is_corpse_loot_inventory(
        &world,
        corpse_id,
        corpse.inventory_id.unwrap()
    ));
    assert!(is_corpse_loot_inventory(
        &world,
        corpse_id,
        equipment.weapon
    ));
    assert!(is_corpse_loot_inventory(&world, corpse_id, internal));
    let unrelated_backpack = create_unique(&mut world, ctx, "test_backpack");
    let unrelated_internal = world
        .item_instance_store()
        .get(unrelated_backpack)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    assert!(!is_corpse_loot_inventory(
        &world,
        corpse_id,
        unrelated_internal
    ));
}

#[test]
fn player_loots_corpse_weapon_to_personal_inventory() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let victim = spawn_bandit(&mut world, ctx);
    let looter = spawn_bandit(&mut world, ctx);
    let personal = victim.inventory_id.unwrap();
    let weapon_slot = victim.equipment.unwrap().weapon;
    let looter_personal = looter.inventory_id.unwrap();
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
    let corpse_id = kill_unit(&mut world, ctx, victim.id);
    let corpse_weapon = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .equipment
        .unwrap()
        .weapon;
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            corpse_weapon,
            0,
            sword_id,
            looter_personal,
            TransferPlacementPolicy::MergeThenFirstFit,
        )
        .unwrap();
    }
    assert!(world.item_instance_store().get(sword_id).is_some());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn player_equips_corpse_weapon_directly() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let victim = spawn_bandit(&mut world, ctx);
    let looter = spawn_bandit(&mut world, ctx);
    let personal = victim.inventory_id.unwrap();
    let weapon_slot = victim.equipment.unwrap().weapon;
    let looter_weapon_slot = looter.equipment.unwrap().weapon;
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
    let corpse_id = kill_unit(&mut world, ctx, victim.id);
    let corpse_weapon = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .equipment
        .unwrap()
        .weapon;
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            corpse_weapon,
            0,
            sword_id,
            looter_weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    assert!(world.item_instance_store().get(sword_id).is_some());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn loaded_corpse_backpack_rejects_personal_inventory() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let victim = spawn_bandit(&mut world, ctx);
    let looter = spawn_bandit(&mut world, ctx);
    let personal = victim.inventory_id.unwrap();
    let backpack_slot = victim.equipment.unwrap().backpack;
    let looter_personal = looter.inventory_id.unwrap();
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
    let corpse_id = kill_unit(&mut world, ctx, victim.id);
    let corpse_backpack_slot = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .equipment
        .unwrap()
        .backpack;
    let err = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            corpse_backpack_slot,
            0,
            backpack_id,
            looter_personal,
            TransferPlacementPolicy::MergeThenFirstFit,
        )
        .unwrap_err()
    };
    assert!(matches!(
        err,
        TransferError::Inventory(InventoryError::LoadedContainerInPersonalInventory { .. })
    ));
}

#[test]
fn loaded_corpse_backpack_equips_to_player_backpack_slot() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let victim = spawn_bandit(&mut world, ctx);
    let looter = spawn_bandit(&mut world, ctx);
    let personal = victim.inventory_id.unwrap();
    let backpack_slot = victim.equipment.unwrap().backpack;
    let looter_backpack_slot = looter.equipment.unwrap().backpack;
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
    let corpse_id = kill_unit(&mut world, ctx, victim.id);
    let corpse_backpack_slot = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .equipment
        .unwrap()
        .backpack;
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            corpse_backpack_slot,
            0,
            backpack_id,
            looter_backpack_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    let instance = world.item_instance_store().get(backpack_id).unwrap();
    assert_eq!(instance.contained_inventory_id, Some(internal));
    assert!(world.inventory_store().get(internal).is_some());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn corpse_expiry_with_loaded_backpack_defers_without_orphans() {
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
    let corpse_id = kill_unit(&mut world, ctx, unit.id);
    world
        .corpse_store_mut()
        .get_mut(corpse_id)
        .unwrap()
        .remaining_lifetime_ticks = 0;
    let lifecycle = step_corpse_lifecycle(&mut world, ctx);
    assert!(!lifecycle.expired_corpse_ids.contains(&corpse_id));
    assert!(world.corpse_store().get(corpse_id).is_some());
    assert!(world.item_instance_store().get(backpack_id).is_some());
    assert!(validate_world_inventory_state(&world, ctx).is_ok());
}

#[test]
fn corpse_equipment_save_load_round_trip() {
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
    let corpse_id = kill_unit(&mut world, ctx, unit.id);
    let snapshot = capture_inventory_persistence(&world);
    let mut restored = flat_world();
    restore_inventory_persistence(&mut restored, &snapshot, &ctx).unwrap();
    let corpse = restored.corpse_store().get(corpse_id).unwrap();
    assert!(corpse.equipment.is_some());
    assert!(validate_world_inventory_state(&restored, ctx).is_ok());
    assert!(restored.item_instance_store().get(sword_id).is_some());
}

#[test]
fn validation_rejects_corpse_equipment_owner_mismatch() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let corpse_id = kill_unit(&mut world, ctx, unit.id);
    let equipment = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .equipment
        .unwrap();
    world
        .inventory_store_mut()
        .get_mut(equipment.weapon)
        .unwrap()
        .set_owner(InventoryOwnerRef::CorpseEquipment {
            corpse_id,
            slot: EquipmentSlot::Head,
        });
    let report = validate_world_inventory_state(&world, ctx);
    assert!(!report.is_ok());
}
