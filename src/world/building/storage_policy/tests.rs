//! Storage policy authority tests.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryEntryContents, place_stack_first_fit, remove_entry,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, ItemCategoryId, ItemDefinitionId,
    LocalPosition, WorldData, WorldPosition, apply_player_storage_accept_all,
    apply_player_storage_category_accepted, apply_player_storage_clear_all,
    building_storage_accepts_item, building_storage_policy, create_building_with_inventory,
    starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions,
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

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn building_catalog() -> BuildingCatalog {
    let categories = BuildingCategoryCatalog::default();
    BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap()
}

fn spawn_chest(world: &mut WorldData, catalog: &BuildingCatalog) -> crate::world::BuildingId {
    let created = create_building_with_inventory(
        catalog,
        world,
        &BuildingDefinitionId::new("storage_chest"),
        pos(10.0, 10.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        test_inventory_ctx(),
    )
    .unwrap();
    world.mutate_building(created.id, |record| {
        record.lifecycle_state = BuildingLifecycleState::Complete;
    });
    created.id
}

fn chest_inventory(
    world: &WorldData,
    chest_id: crate::world::BuildingId,
) -> crate::world::InventoryId {
    world
        .get_building(chest_id)
        .and_then(|record| record.inventory_id)
        .expect("chest inventory")
}

#[test]
fn new_storage_chest_accepts_all_categories_by_default() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    let policy = building_storage_policy(&world, chest);
    assert!(policy.accepts_all());
    assert!(building_storage_accepts_item(
        &world,
        chest,
        &ItemDefinitionId::new("prispod"),
        test_inventory_ctx(),
    ));
}

#[test]
fn default_storage_chest_accepts_prispod() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    assert!(building_storage_accepts_item(
        &world,
        chest,
        &ItemDefinitionId::new("prispod"),
        test_inventory_ctx(),
    ));
}

#[test]
fn disabling_food_category_rejects_prispod() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    apply_player_storage_category_accepted(
        &mut world,
        &catalog,
        chest,
        ItemCategoryId::new("food"),
        false,
    )
    .unwrap();
    assert!(!building_storage_accepts_item(
        &world,
        chest,
        &ItemDefinitionId::new("prispod"),
        test_inventory_ctx(),
    ));
}

#[test]
fn disabling_food_does_not_block_iron_ore() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    apply_player_storage_category_accepted(
        &mut world,
        &catalog,
        chest,
        ItemCategoryId::new("food"),
        false,
    )
    .unwrap();
    assert!(building_storage_accepts_item(
        &world,
        chest,
        &ItemDefinitionId::new("iron_ore"),
        test_inventory_ctx(),
    ));
}

#[test]
fn accept_all_restores_default_acceptance() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    apply_player_storage_clear_all(&mut world, &catalog, test_inventory_ctx().categories, chest)
        .unwrap();
    apply_player_storage_accept_all(&mut world, &catalog, chest).unwrap();
    assert!(building_storage_accepts_item(
        &world,
        chest,
        &ItemDefinitionId::new("prispod"),
        test_inventory_ctx(),
    ));
}

#[test]
fn clear_all_rejects_new_inbound_categories() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    apply_player_storage_clear_all(&mut world, &catalog, test_inventory_ctx().categories, chest)
        .unwrap();
    assert!(!building_storage_accepts_item(
        &world,
        chest,
        &ItemDefinitionId::new("prispod"),
        test_inventory_ctx(),
    ));
}

#[test]
fn existing_inventory_remains_after_category_disabled() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    let inventory_id = chest_inventory(&world, chest);
    let ctx = test_inventory_ctx();
    let (store, instances) = world.inventory_runtime_mut();
    place_stack_first_fit(
        store,
        instances,
        ctx,
        inventory_id,
        ItemDefinitionId::new("prispod"),
        2,
    )
    .unwrap();
    apply_player_storage_category_accepted(
        &mut world,
        &catalog,
        chest,
        ItemCategoryId::new("food"),
        false,
    )
    .unwrap();
    let count = world
        .inventory_store()
        .get(inventory_id)
        .map(|record| {
            record
                .placed_entries()
                .iter()
                .filter(|entry| {
                    matches!(
                        &entry.contents,
                        InventoryEntryContents::Stack {
                            item_definition_id,
                            ..
                        } if item_definition_id.as_str() == "prispod"
                    )
                })
                .count()
        })
        .unwrap_or(0);
    assert_eq!(count, 1);
}

#[test]
fn player_manual_insertion_ignores_storage_category_filter() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    apply_player_storage_category_accepted(
        &mut world,
        &catalog,
        chest,
        ItemCategoryId::new("food"),
        false,
    )
    .unwrap();
    assert!(!building_storage_accepts_item(
        &world,
        chest,
        &ItemDefinitionId::new("prispod"),
        test_inventory_ctx(),
    ));
    let inventory_id = chest_inventory(&world, chest);
    let ctx = test_inventory_ctx();
    let (store, instances) = world.inventory_runtime_mut();
    place_stack_first_fit(
        store,
        instances,
        ctx,
        inventory_id,
        ItemDefinitionId::new("prispod"),
        1,
    )
    .expect("manual player placement ignores category filter");
}

#[test]
fn disabled_category_marks_existing_contents_misfiled() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    let inventory_id = chest_inventory(&world, chest);
    let ctx = test_inventory_ctx();
    let (store, instances) = world.inventory_runtime_mut();
    place_stack_first_fit(
        store,
        instances,
        ctx,
        inventory_id,
        ItemDefinitionId::new("prispod"),
        2,
    )
    .unwrap();
    apply_player_storage_category_accepted(
        &mut world,
        &catalog,
        chest,
        ItemCategoryId::new("food"),
        false,
    )
    .unwrap();
    assert!(crate::world::storage_item_is_misfiled(
        &world,
        chest,
        &ItemDefinitionId::new("prispod"),
        ctx,
    ));
}

#[test]
fn removal_of_existing_items_remains_allowed() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let chest = spawn_chest(&mut world, &catalog);
    let chest_inventory_id = chest_inventory(&world, chest);
    let ctx = test_inventory_ctx();
    let (store, instances) = world.inventory_runtime_mut();
    place_stack_first_fit(
        store,
        instances,
        ctx,
        chest_inventory_id,
        ItemDefinitionId::new("prispod"),
        1,
    )
    .unwrap();
    apply_player_storage_category_accepted(
        &mut world,
        &catalog,
        chest,
        ItemCategoryId::new("food"),
        false,
    )
    .unwrap();
    let (store, instances) = world.inventory_runtime_mut();
    remove_entry(store, instances, ctx, chest_inventory_id, 0).expect("remove allowed");
}

#[test]
fn non_storage_building_policy_mutation_is_rejected() {
    let mut world = flat_world();
    let catalog = building_catalog();
    let farm = create_building_with_inventory(
        &catalog,
        &mut world,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(20.0, 20.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        test_inventory_ctx(),
    )
    .unwrap()
    .id;
    let result = apply_player_storage_category_accepted(
        &mut world,
        &catalog,
        farm,
        ItemCategoryId::new("food"),
        false,
    );
    assert!(result.is_err());
}
