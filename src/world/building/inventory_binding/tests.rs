//! Building inventory binding tests (EP4).

use crate::world::building::create_building_with_inventory;
use crate::world::building::inventory_binding::{
    BuildingInventoryBinding, BuildingInventoryBindingDefinition, BuildingInventoryBindingId,
    BuildingInventoryBindingSet, BuildingInventoryBindingStore, BuildingInventoryRole,
    building_inventories_with_role, resolve_building_inventory_binding,
    validate_building_definition_inventory_bindings, validate_operation_inventory_bindings,
    validate_selected_operation_inventory_bindings,
};
use crate::world::inventory::{InventoryCatalogCtx, InventoryEntryContents, InventoryOwnerRef, place_stack_first_fit};
use crate::world::operation::{
    OperationCatalog, OperationCategory, OperationDefinition, OperationDefinitionId,
    OperationInputDefinition,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinition, BuildingDefinitionId, BuildingOwnership,
    BuildingRenderKey, BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, FootprintSpec,
    Heightfield, InventoryProfileCatalog, InventoryProfileId, ItemCatalog, ItemCategoryCatalog,
    ItemDefinitionId, LocalPosition, WorldData, WorldPosition, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions, starter_item_definitions,
    starter_operation_definitions, BuildingCategoryId,
};
use bevy::prelude::{Quat, Vec3};

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
        let items =
            ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn smelter_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("test_smelter"),
        "Test Smelter",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        100,
        30.0,
        FootprintSpec::Circle { radius_meters: 2.0 },
        30.0,
        true,
    )
    .with_inventory_bindings(vec![
        BuildingInventoryBindingDefinition::new(
            "ore_input",
            BuildingInventoryRole::Input,
            InventoryProfileId::new("chest_large"),
        ),
        BuildingInventoryBindingDefinition::new(
            "fuel_input",
            BuildingInventoryRole::Fuel,
            InventoryProfileId::new("chest_small"),
        ),
        BuildingInventoryBindingDefinition::new(
            "metal_output",
            BuildingInventoryRole::Output,
            InventoryProfileId::new("chest_small"),
        ),
    ])
}

#[test]
fn building_may_own_multiple_inventories() {
    let categories = crate::world::BuildingCategoryCatalog::default();
    let catalog =
        BuildingCatalog::from_definitions(vec![smelter_definition()], &categories).unwrap();
    let mut world = flat_world();
    let record = create_building_with_inventory(
        &catalog,
        &mut world,
        &BuildingDefinitionId::new("test_smelter"),
        pos(1.0, 1.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::neutral(),
        None,
        test_ctx(),
    )
    .unwrap();
    let set = world
        .building_inventory_binding_store()
        .get(record.id)
        .expect("bindings");
    assert_eq!(set.len(), 3);
}

#[test]
fn two_bindings_may_share_role() {
    let definition = smelter_definition();
    let inputs = definition
        .inventory_bindings
        .iter()
        .filter(|binding| binding.role == BuildingInventoryRole::Input)
        .count();
    assert_eq!(inputs, 1);
    let bakery = starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "workbench")
        .expect("workbench");
    let input_bindings = bakery
        .inventory_bindings
        .iter()
        .filter(|binding| binding.role == BuildingInventoryRole::Input)
        .count();
    assert_eq!(input_bindings, 3);
}

#[test]
fn binding_ids_resolve_to_inventory_ids() {
    let categories = crate::world::BuildingCategoryCatalog::default();
    let catalog =
        BuildingCatalog::from_definitions(vec![smelter_definition()], &categories).unwrap();
    let mut world = flat_world();
    let record = create_building_with_inventory(
        &catalog,
        &mut world,
        &BuildingDefinitionId::new("test_smelter"),
        pos(2.0, 2.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::neutral(),
        None,
        test_ctx(),
    )
    .unwrap();
    let store = world.building_inventory_binding_store();
    let ore = resolve_building_inventory_binding(
        store,
        record.id,
        &BuildingInventoryBindingId::new("ore_input"),
    )
    .expect("ore");
    let fuel = resolve_building_inventory_binding(
        store,
        record.id,
        &BuildingInventoryBindingId::new("fuel_input"),
    )
    .expect("fuel");
    assert_ne!(ore, fuel);
}

#[test]
fn resolution_does_not_depend_on_array_order() {
    let mut set = BuildingInventoryBindingSet::from_bindings(vec![
        BuildingInventoryBinding::new(
            "z_last",
            BuildingInventoryRole::Output,
            crate::world::InventoryId::new(99),
        ),
        BuildingInventoryBinding::new(
            "a_first",
            BuildingInventoryRole::Input,
            crate::world::InventoryId::new(1),
        ),
    ]);
    set.rebuild_index();
    assert_eq!(
        set.resolve_inventory(&BuildingInventoryBindingId::new("z_last"))
            .map(|id| id.raw()),
        Some(99)
    );
}

#[test]
fn duplicate_binding_ids_fail_validation() {
    let profiles = InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
        .unwrap();
    let definition = BuildingDefinition::new(
        BuildingDefinitionId::new("dup"),
        "Dup",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        100,
        30.0,
        FootprintSpec::Circle { radius_meters: 2.0 },
        30.0,
        true,
    )
    .with_inventory_bindings(vec![
        BuildingInventoryBindingDefinition::new(
            "ore_input",
            BuildingInventoryRole::Input,
            InventoryProfileId::new("chest_small"),
        ),
        BuildingInventoryBindingDefinition::new(
            "ore_input",
            BuildingInventoryRole::Fuel,
            InventoryProfileId::new("chest_small"),
        ),
    ]);
    let issues = validate_building_definition_inventory_bindings(&definition, &profiles);
    assert!(issues
        .iter()
        .any(|issue| issue.message().contains("more than once")));
}

#[test]
fn legacy_single_inventory_migrates_without_losing_contents() {
    let categories = crate::world::BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(starter_building_definitions(), &categories)
        .unwrap();
    let mut world = flat_world();
    let ctx = test_ctx();
    let record = create_building_with_inventory(
        &catalog,
        &mut world,
        &BuildingDefinitionId::new("storage_chest"),
        pos(3.0, 3.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        ctx,
    )
    .unwrap();
    let inventory_id = record.inventory_id.expect("legacy inventory");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        3,
    )
    .unwrap();
    let binding = world
        .building_inventory_binding_store()
        .get(record.id)
        .and_then(|set| set.get(&BuildingInventoryBindingId::new("primary")))
        .expect("legacy binding");
    assert_eq!(binding.inventory_id, inventory_id);
    assert_eq!(
        world
            .inventory_store()
            .get(inventory_id)
            .and_then(|inv| inv.placed_entries().first())
            .and_then(|entry| match &entry.contents {
                InventoryEntryContents::Stack { quantity, .. } => Some(*quantity),
                _ => None,
            }),
        Some(3)
    );
}

#[test]
fn operation_references_missing_binding_fails_validation() {
    let operation = OperationDefinition::new(
        OperationDefinitionId::new("bad"),
        "Bad",
        "Bad op",
        OperationCategory::Processing,
        1_000,
        1,
    )
    .with_inputs(vec![OperationInputDefinition {
        item_id: ItemDefinitionId::new("iron_ore"),
        quantity: 1,
        source_binding: Some(BuildingInventoryBindingId::new("missing_input")),
    }]);
    let issues = validate_operation_inventory_bindings(&operation, &smelter_definition());
    assert!(issues
        .iter()
        .any(|issue| issue.message().contains("unknown binding")));
}

#[test]
fn role_queries_return_all_matches_without_selection() {
    let mut store = BuildingInventoryBindingStore::default();
    store.set(
        crate::world::BuildingId::new(1),
        BuildingInventoryBindingSet::from_bindings(vec![
            BuildingInventoryBinding::new(
                "a",
                BuildingInventoryRole::Input,
                crate::world::InventoryId::new(1),
            ),
            BuildingInventoryBinding::new(
                "b",
                BuildingInventoryRole::Input,
                crate::world::InventoryId::new(2),
            ),
        ]),
    );
    let matches =
        building_inventories_with_role(&store, crate::world::BuildingId::new(1), BuildingInventoryRole::Input);
    assert_eq!(matches.len(), 2);
}

#[test]
fn selected_operation_validates_runtime_binding_resolution() {
    let categories = crate::world::BuildingCategoryCatalog::default();
    let catalog = BuildingCatalog::from_definitions(starter_building_definitions(), &categories)
        .unwrap();
    let operations = OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let mut world = flat_world();
    let record = create_building_with_inventory(
        &catalog,
        &mut world,
        &BuildingDefinitionId::new("smelter"),
        pos(4.0, 4.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::neutral(),
        None,
        test_ctx(),
    )
    .unwrap();
    let definition = catalog.get(&record.definition_id).unwrap();
    let operation = operations
        .get(&OperationDefinitionId::new("smelt_iron"))
        .unwrap();
    validate_selected_operation_inventory_bindings(
        operation,
        definition,
        record.id,
        world.building_inventory_binding_store(),
    )
    .expect("valid bindings");
}

#[test]
fn binding_store_round_trips_through_save_state() {
    let mut store = BuildingInventoryBindingStore::default();
    store.set(
        crate::world::BuildingId::new(5),
        BuildingInventoryBindingSet::from_bindings(vec![BuildingInventoryBinding::new(
            "ore_input",
            BuildingInventoryRole::Input,
            crate::world::InventoryId::new(10),
        )]),
    );
    let exported = store.export_buildings();
    let mut restored = BuildingInventoryBindingStore::default();
    restored.import_buildings(exported);
    assert_eq!(
        restored
            .resolve_inventory(
                crate::world::BuildingId::new(5),
                &BuildingInventoryBindingId::new("ore_input"),
            )
            .map(|id| id.raw()),
        Some(10)
    );
}
