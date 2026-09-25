//! Equipment and container inventory regression tests (Slice 1).

use bevy::prelude::Vec3;

use crate::dev::{capture_inventory_persistence, restore_inventory_persistence};
use crate::world::UnitOwnership;
use crate::world::armor::ArmorProfileId;
use crate::world::equipment::EquipmentSlot;
use crate::world::unit::{UnitDefinitionId, UnitSource};
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryCatalogCtx, InventoryError,
    InventoryOwnerRef, InventoryProfileCatalog, InventoryProfileId, ItemCatalog,
    ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId, ItemDefinition, ItemDefinitionId,
    ItemInstanceMetadata, LocalPosition, TransferPlacementPolicy, UnitCatalog, WeaponDefinitionId,
    WorldData, WorldPosition, create_item_instance, create_unit, create_unit_with_inventory,
    place_unique, place_unique_first_fit, starter_inventory_profile_definitions,
    starter_unit_definitions, transfer_unique_item, validate_unit_equipment_links,
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

fn test_item_catalog() -> ItemCatalog {
    let categories = ItemCategoryCatalog::from_definitions(vec![
        ItemCategoryDefinition::new(ItemCategoryId::new("armor"), "Armor", "", true),
        ItemCategoryDefinition::new(ItemCategoryId::new("weapon"), "Weapon", "", true),
        ItemCategoryDefinition::new(ItemCategoryId::new("container"), "Container", "", true),
        ItemCategoryDefinition::new(ItemCategoryId::new("raw_material"), "Raw", "", true),
    ])
    .unwrap();
    let items = vec![
        ItemDefinition::new(
            ItemDefinitionId::new("test_helmet"),
            "Test Helmet",
            "",
            ItemCategoryId::new("armor"),
            2,
            2,
            false,
            1,
            1_000,
            1,
            true,
        )
        .with_unique_instance_required(true)
        .with_equipment_slots(vec![EquipmentSlot::Head])
        .with_armor_profile_id(ArmorProfileId::new("armor_test_head")),
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
        .with_weapon_definition_id(WeaponDefinitionId::new("weapon_test_sword")),
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
    ];
    ItemCatalog::from_definitions(items, &categories).unwrap()
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
        let items = Box::leak(Box::new(test_item_catalog()));
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
        UnitSource::Authored,
        UnitOwnership::hostile(),
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

#[test]
fn unit_creation_allocates_personal_and_eight_equipment_inventories() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    assert!(unit.inventory_id.is_some());
    let equipment = unit.equipment.expect("equipment inventories");
    assert_ne!(equipment.head, equipment.weapon);
    validate_unit_equipment_links(&world, unit.id).unwrap();
}

#[test]
fn equipment_inventories_have_semantic_owner_and_profile() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let equipment = unit.equipment.unwrap();
    for slot in EquipmentSlot::ALL {
        let inventory_id = equipment.inventory_id(slot);
        let record = world.inventory_store().get(inventory_id).unwrap();
        assert_eq!(
            record.owner(),
            &InventoryOwnerRef::UnitEquipment {
                unit_id: unit.id,
                slot,
            }
        );
        assert_eq!(record.profile_id(), &slot.profile_id());
        let profile = ctx.require_profile(record.profile_id()).unwrap();
        assert_eq!(profile.equipment_slot, Some(slot));
        assert_eq!(profile.max_placed_entries, Some(1));
    }
}

#[test]
fn valid_equip_transfer_preserves_item_instance_id() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let instance_id = create_unique(&mut world, &ctx, "test_sword");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, instance_id)
            .unwrap();
    }
    let source_entry = world
        .inventory_store()
        .get(personal)
        .unwrap()
        .placed_entries()[0]
        .clone();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    transfer_unique_item(
        inventory_store,
        instance_store,
        &ctx,
        personal,
        0,
        instance_id,
        weapon_slot,
        TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
    )
    .unwrap();
    assert!(
        world
            .inventory_store()
            .get(personal)
            .unwrap()
            .placed_entries()
            .is_empty()
    );
    let equipped = world.inventory_store().get(weapon_slot).unwrap();
    assert_eq!(equipped.placed_entries()[0].contents, source_entry.contents);
    assert_eq!(
        world.item_instance_store().location(instance_id),
        Some(crate::world::ItemInstanceLocation::Inventory {
            inventory_id: weapon_slot,
            entry_index: 0,
        })
    );
}

#[test]
fn incompatible_equipment_transfer_rejects_transactionally() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let instance_id = create_unique(&mut world, &ctx, "test_helmet");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, instance_id)
            .unwrap();
    }
    let personal_before = world.inventory_store().get(personal).unwrap().clone();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let err = transfer_unique_item(
        inventory_store,
        instance_store,
        &ctx,
        personal,
        0,
        instance_id,
        weapon_slot,
        TransferPlacementPolicy::FirstFitOnly,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::world::TransferError::Inventory(InventoryError::IncompatibleEquipmentSlot { .. })
    ));
    assert_eq!(
        world.inventory_store().get(personal).unwrap(),
        &personal_before
    );
}

#[test]
fn occupied_equipment_slot_rejects_second_item() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword_a = create_unique(&mut world, &ctx, "test_sword");
    let sword_b = create_unique(&mut world, &ctx, "test_sword");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, sword_a).unwrap();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, sword_b).unwrap();
    }
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            sword_a,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    let weapon_before = world.inventory_store().get(weapon_slot).unwrap().clone();
    let sword_b_index = world
        .inventory_store()
        .get(personal)
        .unwrap()
        .placed_entries()
        .iter()
        .position(|entry| {
            matches!(
                entry.contents,
                crate::world::InventoryEntryContents::Unique { item_instance_id } if item_instance_id == sword_b
            )
        })
        .expect("sword_b in personal inventory");
    let err = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            sword_b_index,
            sword_b,
            weapon_slot,
            TransferPlacementPolicy::FirstFitOnly,
        )
        .unwrap_err()
    };
    assert!(matches!(
        err,
        crate::world::TransferError::Inventory(InventoryError::MaxPlacedEntriesExceeded { .. })
            | crate::world::TransferError::DestinationNoFit
    ));
    assert_eq!(
        world.inventory_store().get(weapon_slot).unwrap(),
        &weapon_before
    );
}

#[test]
fn equipment_slot_uses_real_item_footprint() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let weapon_slot = unit.equipment.unwrap().weapon;
    let instance_id = create_unique(&mut world, &ctx, "test_sword");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique(
            inventory_store,
            instance_store,
            &ctx,
            weapon_slot,
            instance_id,
            0,
            0,
        )
        .unwrap();
    }
    let entry = world
        .inventory_store()
        .get(weapon_slot)
        .unwrap()
        .placed_entries()[0]
        .clone();
    assert_eq!(entry.anchor_x, 0);
    assert_eq!(entry.anchor_y, 0);
    let helmet = create_unique(&mut world, &ctx, "test_helmet");
    let err = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique(
            inventory_store,
            instance_store,
            &ctx,
            weapon_slot,
            helmet,
            0,
            2,
        )
    };
    assert!(err.is_err());
}

#[test]
fn unequip_to_personal_when_room_exists() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let instance_id = create_unique(&mut world, &ctx, "test_sword");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_unique(
        inventory_store,
        instance_store,
        &ctx,
        weapon_slot,
        instance_id,
        0,
        0,
    )
    .unwrap();
    transfer_unique_item(
        inventory_store,
        instance_store,
        &ctx,
        weapon_slot,
        0,
        instance_id,
        personal,
        TransferPlacementPolicy::FirstFitOnly,
    )
    .unwrap();
    assert_eq!(
        world
            .item_instance_store()
            .location(instance_id)
            .unwrap()
            .inventory()
            .map(|(id, _)| id),
        Some(personal)
    );
}

#[test]
fn unequip_rejects_when_personal_inventory_has_no_fit() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword = create_unique(&mut world, &ctx, "test_sword");
    let fillers = (0..36)
        .map(|_| create_unique(&mut world, &ctx, "test_small_item"))
        .collect::<Vec<_>>();
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique(
            inventory_store,
            instance_store,
            &ctx,
            weapon_slot,
            sword,
            0,
            0,
        )
        .unwrap();
        for (index, filler) in fillers.iter().enumerate() {
            let index = u8::try_from(index).unwrap();
            place_unique(
                inventory_store,
                instance_store,
                &ctx,
                personal,
                *filler,
                index % 6,
                index / 6,
            )
            .unwrap();
        }
    }
    let weapon_before = world.inventory_store().get(weapon_slot).unwrap().clone();
    let err = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            weapon_slot,
            0,
            sword,
            personal,
            TransferPlacementPolicy::FirstFitOnly,
        )
        .unwrap_err()
    };
    assert!(matches!(err, crate::world::TransferError::DestinationNoFit));
    assert_eq!(
        world.inventory_store().get(weapon_slot).unwrap(),
        &weapon_before
    );
}

#[test]
fn backpack_instance_creates_one_internal_inventory() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let instance_id = create_unique(&mut world, &ctx, "test_backpack");
    let instance = world.item_instance_store().get(instance_id).unwrap();
    let internal = instance.contained_inventory_id.expect("internal inventory");
    let record = world.inventory_store().get(internal).unwrap();
    assert_eq!(
        record.owner(),
        &InventoryOwnerRef::ItemContainer(instance_id)
    );
}

#[test]
fn empty_backpack_may_enter_personal_inventory() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let instance_id = create_unique(&mut world, &ctx, "test_backpack");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_unique_first_fit(inventory_store, instance_store, &ctx, personal, instance_id).unwrap();
    assert_eq!(
        world
            .item_instance_store()
            .location(instance_id)
            .unwrap()
            .inventory()
            .map(|(id, _)| id),
        Some(personal)
    );
}

#[test]
fn loaded_backpack_cannot_enter_personal_inventory() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_id = create_unique(&mut world, &ctx, "test_backpack");
    let internal = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let filler = create_unique(&mut world, &ctx, "test_small_item");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_unique(
        inventory_store,
        instance_store,
        &ctx,
        internal,
        filler,
        0,
        0,
    )
    .unwrap();
    let err = place_unique_first_fit(inventory_store, instance_store, &ctx, personal, backpack_id)
        .unwrap_err();
    assert!(matches!(
        err,
        InventoryError::LoadedContainerInPersonalInventory { .. }
    ));
}

#[test]
fn cannot_load_backpack_while_in_personal_inventory() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_id = create_unique(&mut world, &ctx, "test_backpack");
    let internal = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let filler = create_unique(&mut world, &ctx, "test_small_item");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, backpack_id)
            .unwrap();
    }
    let backpack_before = world.inventory_store().get(internal).unwrap().clone();
    let filler_location_before = world.item_instance_store().location(filler);
    let err = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique(
            inventory_store,
            instance_store,
            &ctx,
            internal,
            filler,
            0,
            0,
        )
        .unwrap_err()
    };
    assert!(matches!(
        err,
        InventoryError::ContainerLoadingForbiddenInPersonalInventory { .. }
    ));
    assert_eq!(
        world.inventory_store().get(internal).unwrap(),
        &backpack_before
    );
    assert_eq!(
        world.item_instance_store().location(filler),
        filler_location_before
    );
}

#[test]
fn loaded_backpack_may_equip_to_backpack_slot() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let backpack_id = create_unique(&mut world, &ctx, "test_backpack");
    let internal = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let filler = create_unique(&mut world, &ctx, "test_small_item");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, backpack_id)
            .unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            backpack_id,
            backpack_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        place_unique(
            inventory_store,
            instance_store,
            &ctx,
            internal,
            filler,
            0,
            0,
        )
        .unwrap();
    }
    assert_eq!(
        world
            .inventory_store()
            .get(backpack_slot)
            .unwrap()
            .placed_entries()
            .len(),
        1
    );
    assert_eq!(
        world
            .inventory_store()
            .get(internal)
            .unwrap()
            .placed_entries()
            .len(),
        1
    );
}

#[test]
fn backpack_cannot_be_nested_in_another_backpack() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let outer = create_unique(&mut world, &ctx, "test_backpack");
    let inner = create_unique(&mut world, &ctx, "test_backpack");
    let outer_internal = world
        .item_instance_store()
        .get(outer)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let err = place_unique_first_fit(inventory_store, instance_store, &ctx, outer_internal, inner)
        .unwrap_err();
    assert!(matches!(
        err,
        InventoryError::ContainerRecursionForbidden { .. }
    ));
}

#[test]
fn moving_backpack_preserves_internal_inventory_id() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let backpack_id = create_unique(&mut world, &ctx, "test_backpack");
    let internal_before = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id;
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_unique_first_fit(inventory_store, instance_store, &ctx, personal, backpack_id).unwrap();
    transfer_unique_item(
        inventory_store,
        instance_store,
        &ctx,
        personal,
        0,
        backpack_id,
        backpack_slot,
        TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
    )
    .unwrap();
    let internal_after = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id;
    assert_eq!(internal_before, internal_after);
}

#[test]
fn equipment_and_backpack_state_survives_save_load_roundtrip() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let backpack_slot = unit.equipment.unwrap().backpack;
    let sword = create_unique(&mut world, &ctx, "test_sword");
    let backpack = create_unique(&mut world, &ctx, "test_backpack");
    let internal = world
        .item_instance_store()
        .get(backpack)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let filler = create_unique(&mut world, &ctx, "test_small_item");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, sword).unwrap();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, backpack).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            sword,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            backpack,
            backpack_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        place_unique(
            inventory_store,
            instance_store,
            &ctx,
            internal,
            filler,
            0,
            0,
        )
        .unwrap();
    }

    let unit_record = world.get_unit(unit.id).cloned().expect("unit");
    let persistence = capture_inventory_persistence(&world);
    let mut restored = flat_world();
    let chunk = ChunkId::new(ChunkCoord::new(0, 0));
    restored
        .insert_unit(chunk, unit_record)
        .expect("restore unit record");
    restore_inventory_persistence(
        &mut restored,
        &persistence,
        &ctx,
        &crate::world::AppearanceProfileCatalog::empty(),
    )
    .unwrap();
    let report = validate_world_inventory_state(&restored, &ctx);
    assert!(
        report.is_ok(),
        "validation failed: {:?}",
        report.link_errors
    );
    assert_eq!(
        restored
            .item_instance_store()
            .get(backpack)
            .unwrap()
            .contained_inventory_id,
        Some(internal)
    );
}

#[test]
fn wolf_without_personal_inventory_still_gets_equipment() {
    let mut world = flat_world();
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = create_unit(
        &catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(3.0, 3.0),
        UnitSource::Authored,
    )
    .unwrap();
    assert!(unit.inventory_id.is_none());
    assert!(unit.equipment.is_some());
    validate_unit_equipment_links(&world, unit.id).unwrap();
}
