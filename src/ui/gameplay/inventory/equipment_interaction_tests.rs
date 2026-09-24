//! Equipment transfer tests through authoritative inventory APIs (Slice 2).

use bevy::prelude::Vec3;

use crate::ui::gameplay::inventory::equipment_ui::resolve_unit_equipment_ui;
use crate::world::{
    BuildingCatalog, BuildingInteractionProfileCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
    Heightfield, InventoryAccessResult, InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, LocalPosition, TransferPlacementPolicy, UnitCatalog, UnitDefinitionId,
    UnitOwnership, UnitSource, WorldData, WorldPosition, can_unit_access_inventory,
    create_item_instance, create_unit_with_inventory, place_unique, place_unique_first_fit,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_unit_definitions, transfer_entry_full, transfer_unique_item,
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
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let categories = Box::leak(Box::new(categories));
        let items = Box::leak(Box::new(items));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn bandit(world: &mut WorldData, ctx: &InventoryCatalogCtx<'_>) -> crate::world::UnitRecord {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    create_unit_with_inventory(
        &catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        ctx,
    )
    .unwrap()
}

fn transfer_cell(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    source: crate::world::InventoryId,
    entry_index: usize,
    destination: crate::world::InventoryId,
) -> Result<crate::world::TransferReport, crate::world::TransferError> {
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    transfer_entry_full(
        inventory_store,
        instance_store,
        ctx,
        source,
        entry_index,
        destination,
        TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
    )
}

#[test]
fn equip_moves_same_instance_to_weapon_slot() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let building_catalog = BuildingCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    assert!(matches!(
        can_unit_access_inventory(
            &world,
            &building_catalog,
            &interaction,
            unit.id,
            weapon_slot
        ),
        crate::world::InventoryAccessResult::Allowed
    ));
    let sword_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("iron_sword"),
            Default::default(),
        )
        .unwrap()
    };
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, sword_id).unwrap();
    }
    transfer_cell(&mut world, ctx, personal, 0, weapon_slot).unwrap();
    assert_eq!(
        world
            .item_instance_store()
            .location(sword_id)
            .and_then(|loc| loc.inventory().map(|(id, _)| id)),
        Some(weapon_slot)
    );
}

#[test]
fn incompatible_equip_rejects() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let helmet_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("ranger_hood"),
            Default::default(),
        )
        .unwrap()
    };
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, helmet_id).unwrap();
    }
    assert!(transfer_cell(&mut world, ctx, personal, 0, weapon_slot).is_err());
}

#[test]
fn occupied_weapon_slot_rejects_second_item() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let first = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("iron_sword"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            id,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        id
    };
    let second = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("iron_sword"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, id).unwrap();
        id
    };
    assert!(transfer_cell(&mut world, ctx, personal, 0, weapon_slot).is_err());
    assert_eq!(
        world
            .item_instance_store()
            .location(second)
            .and_then(|l| l.inventory().map(|(id, _)| id)),
        Some(personal)
    );
    assert_eq!(
        world
            .item_instance_store()
            .location(first)
            .and_then(|l| l.inventory().map(|(id, _)| id)),
        Some(weapon_slot)
    );
}

#[test]
fn unequip_to_personal_when_fit_succeeds() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("iron_sword"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            id,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        id
    };
    transfer_cell(&mut world, ctx, weapon_slot, 0, personal).unwrap();
    assert_eq!(
        world
            .item_instance_store()
            .location(sword_id)
            .and_then(|l| l.inventory().map(|(id, _)| id)),
        Some(personal)
    );
}

#[test]
fn unequip_to_personal_fails_when_no_fit() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("iron_sword"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            id,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        id
    };
    let blocker = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("ranger_body"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, id).unwrap();
        id
    };
    assert!(transfer_cell(&mut world, ctx, weapon_slot, 0, personal).is_err());
    assert_eq!(
        world
            .item_instance_store()
            .location(sword_id)
            .and_then(|l| l.inventory().map(|(id, _)| id)),
        Some(weapon_slot)
    );
    assert_eq!(
        world
            .item_instance_store()
            .location(blocker)
            .and_then(|l| l.inventory().map(|(id, _)| id)),
        Some(personal)
    );
}

#[test]
fn loaded_equipped_backpack_unequip_rejects() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let backpack_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("leather_backpack"),
            Default::default(),
        )
        .unwrap()
    };
    let internal = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let filler = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("healing_kit"),
            Default::default(),
        )
        .unwrap()
    };
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
    assert!(transfer_cell(&mut world, ctx, backpack_slot, 0, personal).is_err());
    assert_eq!(
        world
            .item_instance_store()
            .location(backpack_id)
            .and_then(|l| l.inventory().map(|(id, _)| id)),
        Some(backpack_slot)
    );
}

#[test]
fn empty_equipped_backpack_unequip_clears_pane_resolution() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let backpack_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("leather_backpack"),
            Default::default(),
        )
        .unwrap()
    };
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
    assert!(
        resolve_unit_equipment_ui(&world, unit.id)
            .unwrap()
            .backpack_internal
            .is_some()
    );
    transfer_cell(&mut world, ctx, backpack_slot, 0, personal).unwrap();
    assert!(
        resolve_unit_equipment_ui(&world, unit.id)
            .unwrap()
            .backpack_internal
            .is_none()
    );
}

#[test]
fn personal_to_equipped_backpack_internal_succeeds() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, ctx);
    let personal = unit.inventory_id.unwrap();
    let backpack_slot = unit.equipment.unwrap().backpack;
    let backpack_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("leather_backpack"),
            Default::default(),
        )
        .unwrap()
    };
    let internal = world
        .item_instance_store()
        .get(backpack_id)
        .unwrap()
        .contained_inventory_id
        .unwrap();
    let kit = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            crate::world::ItemDefinitionId::new("healing_kit"),
            Default::default(),
        )
        .unwrap()
    };
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
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, kit).unwrap();
    }
    transfer_cell(&mut world, ctx, personal, 0, internal).unwrap();
    assert_eq!(
        world
            .item_instance_store()
            .location(kit)
            .and_then(|l| l.inventory().map(|(id, _)| id)),
        Some(internal)
    );
}
