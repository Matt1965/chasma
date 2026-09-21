use bevy::prelude::{Quat, Vec3};

use crate::world::building::operation::{
    BuildingOperationPolicy, ControlSource, OperationDefinitionId, OperationLifecycle,
    ProductionProgress, RepeatMode,
};
use crate::world::inventory::{
    InventoryCatalogCtx, ItemInstanceMetadata, create_item_instance, place_stack_first_fit,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, ItemCatalog, ItemCategoryCatalog,
    ItemCategoryId, ItemDefinitionId, LocalPosition, WorldData, WorldPosition,
    apply_player_storage_category_accepted, build_building_archetype_definition,
    capture_building_archetype_members, capture_building_durable_extensions,
    create_building, create_building_with_inventory, inventory_subgraph_item_count,
    load_building_archetype_catalog_from_ron, save_building_archetype_catalog_to_ron,
    starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, test_equipment_fixture_definitions,
    validate_building_archetype_definition,
};

fn layout_world() -> WorldData {
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
        let mut items = starter_item_definitions();
        items.extend(test_equipment_fixture_definitions());
        let items = ItemCatalog::from_definitions(items, &categories).unwrap();
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

fn building_catalog() -> crate::world::BuildingCatalog {
    let categories = BuildingCategoryCatalog::default();
    crate::world::BuildingCatalog::from_definitions(starter_building_definitions(), &categories)
        .unwrap()
}

fn spawn_hut(world: &mut WorldData, x: f32, z: f32) -> crate::world::BuildingRecord {
    create_building(
        &building_catalog(),
        world,
        &BuildingDefinitionId::new("hut"),
        pos(x, z),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap()
}

fn spawn_chest(world: &mut WorldData, x: f32, z: f32) -> crate::world::BuildingRecord {
    let created = create_building_with_inventory(
        &building_catalog(),
        world,
        &BuildingDefinitionId::new("storage_chest"),
        pos(x, z),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        test_inventory_ctx(),
    )
    .unwrap();
    world.mutate_building(created.id, |record| {
        record.lifecycle_state = BuildingLifecycleState::Complete;
        record.container_locked = true;
    });
    world.get_building(created.id).unwrap().clone()
}

#[test]
fn root_without_inventory_has_empty_extensions() {
    let mut world = layout_world();
    let root = spawn_hut(&mut world, 50.0, 50.0);
    let extensions = capture_building_durable_extensions(&world, &root);
    assert!(extensions.inventory.is_none());
    assert!(extensions.operation_policy.is_none());
    assert!(extensions.storage_policy.is_none());
}

#[test]
fn captured_member_building_includes_inventory_and_lock_state() {
    let mut world = layout_world();
    let root = spawn_hut(&mut world, 50.0, 50.0);
    let chest = spawn_chest(&mut world, 52.0, 50.0);
    let inventory_id = chest.inventory_id.expect("chest inventory");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            ItemDefinitionId::new("iron_ore"),
            12,
        )
        .unwrap();
    }
    let (_, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    assert_eq!(members.len(), 1);
    let state = members[0].building_state.as_ref().expect("building state");
    assert!(state.container_locked);
    let inventory = state.extensions.inventory.as_ref().expect("inventory");
    assert_eq!(inventory_subgraph_item_count(inventory), 1);
}

#[test]
fn operation_policy_captured_without_runtime_progress() {
    let mut world = layout_world();
    let workbench = create_building_with_inventory(
        &building_catalog(),
        &mut world,
        &BuildingDefinitionId::new("workbench"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        test_inventory_ctx(),
    )
    .unwrap();
    world.building_production_store_mut().set_policy(
        workbench.id,
        BuildingOperationPolicy {
            enabled: true,
            paused: false,
            selected_operation: Some(OperationDefinitionId::new("bake_bread")),
            repeat_mode: RepeatMode::Count(3),
            priority: 80,
            control_source: ControlSource::PlayerControlled,
            planner_managed: false,
        },
    );
    {
        let state = world.building_production_store_mut().get_or_default_mut(workbench.id);
        state.lifecycle = OperationLifecycle::Running;
        state.progress = ProductionProgress(42_000);
        state.active_worker_count = 2;
    }
    let extensions = capture_building_durable_extensions(&world, &workbench);
    let policy = extensions.operation_policy.as_ref().expect("policy");
    assert_eq!(
        policy.selected_operation.as_ref().map(|id| id.as_str()),
        Some("bake_bread")
    );
    let serialized = ron::ser::to_string(&extensions).unwrap();
    assert!(!serialized.contains("progress"));
    assert!(!serialized.contains("active_worker_count"));
    assert!(!serialized.contains("lifecycle"));
}

#[test]
fn storage_policy_captured_when_authored() {
    let mut world = layout_world();
    let chest = spawn_chest(&mut world, 50.0, 50.0);
    apply_player_storage_category_accepted(
        &mut world,
        &building_catalog(),
        chest.id,
        ItemCategoryId::new("food"),
        false,
    )
    .unwrap();
    let extensions = capture_building_durable_extensions(&world, &chest);
    let policy = extensions.storage_policy.as_ref().expect("storage policy");
    assert!(!policy.accepts_category(&ItemCategoryId::new("food")));
}

#[test]
fn nested_inventory_round_trips_through_ron() {
    let mut world = layout_world();
    let root = spawn_hut(&mut world, 50.0, 50.0);
    let chest = spawn_chest(&mut world, 52.0, 50.0);
    let inventory_id = chest.inventory_id.expect("inventory");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let backpack = create_item_instance(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            ItemDefinitionId::new("leather_backpack"),
            ItemInstanceMetadata::default(),
        )
        .unwrap();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            ItemDefinitionId::new("iron_ore"),
            5,
        )
        .unwrap();
        crate::world::inventory::place_unique_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            backpack,
        )
        .unwrap();
        let internal = instance_store
            .get(backpack)
            .unwrap()
            .contained_inventory_id
            .expect("internal inventory");
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            internal,
            ItemDefinitionId::new("prispod"),
            2,
        )
        .unwrap();
    }
    let (metadata, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    let definition = build_building_archetype_definition(
        crate::world::BuildingArchetypeId::new("nested_shop"),
        "Nested Shop".to_string(),
        &root,
        &world,
        metadata,
        members,
        true,
    );
    let dir = std::env::temp_dir().join("chasma_building_archetype_nested");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("building_archetypes.ron");
    let catalog =
        crate::world::BuildingArchetypeCatalog::from_definitions(vec![definition]).unwrap();
    save_building_archetype_catalog_to_ron(&catalog, &path).unwrap();
    let loaded = load_building_archetype_catalog_from_ron(&path).unwrap();
    let member = &loaded.definitions()[0].members[0];
    let inventory = member
        .building_state
        .as_ref()
        .and_then(|state| state.extensions.inventory.as_ref())
        .expect("nested inventory");
    assert_eq!(inventory.inventories.len(), 2);
    assert_eq!(inventory.item_instances.len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn save_over_replaces_inventory_snapshot() {
    let mut world = layout_world();
    let root = spawn_hut(&mut world, 50.0, 50.0);
    let chest = spawn_chest(&mut world, 52.0, 50.0);
    let inventory_id = chest.inventory_id.expect("inventory");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            ItemDefinitionId::new("iron_ore"),
            4,
        )
        .unwrap();
    }
    let (metadata, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    let first = build_building_archetype_definition(
        crate::world::BuildingArchetypeId::new("replace_test"),
        "Replace".to_string(),
        &root,
        &world,
        metadata,
        members,
        true,
    );
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let _ = crate::world::inventory::remove_entry(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            0,
        );
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            ItemDefinitionId::new("bread"),
            7,
        )
        .unwrap();
    }
    let (metadata, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    let second = build_building_archetype_definition(
        crate::world::BuildingArchetypeId::new("replace_test"),
        "Replace".to_string(),
        &root,
        &world,
        metadata,
        members,
        true,
    );
    assert_eq!(first.id, second.id);
    let inventory = second.members[0]
        .building_state
        .as_ref()
        .and_then(|state| state.extensions.inventory.as_ref())
        .expect("inventory");
    assert!(inventory.inventories[0].entries.iter().any(|entry| {
        entry.item_definition_id.as_deref() == Some("bread")
    }));
}

#[test]
fn validation_rejects_unknown_item_in_inventory() {
    let mut world = layout_world();
    let root = spawn_hut(&mut world, 50.0, 50.0);
    let (metadata, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    let mut definition = build_building_archetype_definition(
        crate::world::BuildingArchetypeId::new("invalid"),
        "Invalid".to_string(),
        &root,
        &world,
        metadata,
        members,
        true,
    );
    definition.snapshot.extensions.inventory = Some(crate::world::InventorySubgraphSnapshot {
        root_inventory_local_id: 1,
        inventories: vec![crate::world::InventorySubgraphInventory {
            local_id: 1,
            profile_id: "chest_small".into(),
            grid_width: 4,
            grid_height: 4,
            entries: vec![crate::world::InventorySubgraphPlacedEntry {
                anchor_x: 0,
                anchor_y: 0,
                entry_kind: "stack".into(),
                item_definition_id: Some("not_a_real_item".into()),
                item_instance_local_id: None,
                quantity: Some(1),
            }],
        }],
        item_instances: Vec::new(),
        item_instance_locations: Vec::new(),
    });
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let result = validate_building_archetype_definition(
        &definition,
        &items,
        &building_catalog(),
        &crate::world::DoodadCatalog::default(),
        &crate::world::OperationCatalog::default(),
    );
    assert!(result.is_err());
}
