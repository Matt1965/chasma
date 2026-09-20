use std::sync::OnceLock;

use bevy::prelude::{Quat, Vec3};

use crate::world::inventory::{
    InventoryCatalogCtx, InventoryOwnerRef, ItemInstanceMetadata, create_inventory,
    create_item_instance, place_stack_first_fit, place_unique_first_fit,
};
use crate::world::item_pile::{DropReport, ItemPileSettings, PileOwnership, drop_stack_from_inventory};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkData, ChunkExtent, ChunkId, Heightfield,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, LocalPosition,
    SpaceId, WorldData, WorldPosition, build_building_archetype_definition,
    capture_building_archetype_members, create_building, create_building_with_inventory,
    load_building_archetype_catalog_from_ron, save_building_archetype_catalog_to_ron,
    starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, test_equipment_fixture_definitions,
};

use super::building::BuildingArchetypeMemberKind;

fn layout_world() -> WorldData {
    let layout = crate::world::WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
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

fn test_inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: OnceLock<InventoryCatalogCtx<'static>> = OnceLock::new();
    CTX.get_or_init(|| {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let mut items = starter_item_definitions();
        items.extend(test_equipment_fixture_definitions());
        let items = ItemCatalog::from_definitions(items, &categories).unwrap();
        let profiles = InventoryProfileCatalog::from_definitions(
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

fn spawn_root(world: &mut WorldData) -> crate::world::BuildingRecord {
    create_building(
        &building_catalog(),
        world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap()
}

fn drop_stack_at(world: &mut WorldData, x: f32, z: f32, quantity: u32) -> DropReport {
    let ctx = test_inventory_ctx();
    let settings = ItemPileSettings::default();
    let inventory_id = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        quantity,
    )
    .unwrap();
    drop_stack_from_inventory(
        world,
        ctx,
        &settings,
        inventory_id,
        0,
        quantity,
        pos(x, z),
        SpaceId::SURFACE,
        PileOwnership {
            owner_id: None,
            team_id: None,
            affiliation: Affiliation::Player,
        },
        1,
    )
    .unwrap()
}

#[test]
fn loose_item_inside_margin_captured() {
    let mut world = layout_world();
    let root = spawn_root(&mut world);
    drop_stack_at(&mut world, 52.0, 50.0, 3);
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
    assert_eq!(members[0].kind, BuildingArchetypeMemberKind::WorldItemPile);
    assert_eq!(members[0].definition_id, "iron_ore");
    assert_eq!(members[0].world_item_state.as_ref().unwrap().stack_quantity, Some(3));
}

#[test]
fn loose_item_outside_margin_excluded() {
    let mut world = layout_world();
    let root = spawn_root(&mut world);
    drop_stack_at(&mut world, 90.0, 90.0, 1);
    let (_, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    assert!(members.is_empty());
}

#[test]
fn inventory_item_not_captured_as_spatial_member() {
    let mut world = layout_world();
    let root = spawn_root(&mut world);
    let chest = create_building_with_inventory(
        &building_catalog(),
        &mut world,
        &BuildingDefinitionId::new("storage_chest"),
        pos(52.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        test_inventory_ctx(),
    )
    .unwrap();
    world.mutate_building(chest.id, |record| {
        record.lifecycle_state = BuildingLifecycleState::Complete;
    });
    let inventory_id = chest.inventory_id.expect("chest inventory");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            ItemDefinitionId::new("bread"),
            4,
        )
        .unwrap();
    }
    drop_stack_at(&mut world, 53.0, 50.0, 1);
    let (_, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    assert_eq!(members.len(), 2);
    assert!(members.iter().any(|member| member.kind == BuildingArchetypeMemberKind::Building));
    assert_eq!(
        members
            .iter()
            .filter(|member| member.kind == BuildingArchetypeMemberKind::WorldItemPile)
            .count(),
        1
    );
    assert!(members
        .iter()
        .all(|member| member.kind != BuildingArchetypeMemberKind::WorldItemPile || member.definition_id != "bread"));
    let chest_member = members
        .iter()
        .find(|member| member.kind == BuildingArchetypeMemberKind::Building)
        .expect("chest member");
    assert!(chest_member
        .building_state
        .as_ref()
        .and_then(|state| state.extensions.inventory.as_ref())
        .is_some());
}

#[test]
fn unique_world_item_with_nested_inventory_captured() {
    let mut world = layout_world();
    let root = spawn_root(&mut world);
    let ctx = test_inventory_ctx();
    let inventory_id = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let backpack = create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            ItemDefinitionId::new("leather_backpack"),
            ItemInstanceMetadata::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, ctx, inventory_id, backpack)
            .unwrap();
        let internal = instance_store
            .get(backpack)
            .unwrap()
            .contained_inventory_id
            .expect("internal inventory");
        place_stack_first_fit(
            inventory_store,
            instance_store,
            ctx,
            internal,
            ItemDefinitionId::new("prispod"),
            2,
        )
        .unwrap();
    }
    crate::world::item_pile::drop_unique_from_inventory(
        &mut world,
        ctx,
        inventory_id,
        0,
        pos(52.0, 50.0),
        SpaceId::SURFACE,
        PileOwnership {
            owner_id: None,
            team_id: None,
            affiliation: Affiliation::Player,
        },
        1,
    )
    .unwrap();
    let (_, members) = capture_building_archetype_members(
        &world,
        &root,
        &building_catalog(),
        &crate::world::FootprintCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    let pile_member = members
        .iter()
        .find(|member| member.kind == BuildingArchetypeMemberKind::WorldItemPile)
        .expect("world item member");
    assert_eq!(pile_member.definition_id, "leather_backpack");
    let inventory = pile_member
        .world_item_state
        .as_ref()
        .and_then(|state| state.unique_inventory.as_ref())
        .expect("nested inventory");
    assert_eq!(inventory.inventories.len(), 1);
}

#[test]
fn world_item_members_round_trip_through_ron() {
    let mut world = layout_world();
    let root = spawn_root(&mut world);
    drop_stack_at(&mut world, 52.0, 50.0, 5);
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
        crate::world::BuildingArchetypeId::new("shop_display"),
        "Shop Display".to_string(),
        &root,
        &world,
        metadata,
        members,
        true,
    );
    let dir = std::env::temp_dir().join("chasma_world_item_archetype");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("building_archetypes.ron");
    let catalog =
        crate::world::BuildingArchetypeCatalog::from_definitions(vec![definition]).unwrap();
    save_building_archetype_catalog_to_ron(&catalog, &path).unwrap();
    let loaded = load_building_archetype_catalog_from_ron(&path).unwrap();
    let member = &loaded.definitions()[0].members[0];
    assert_eq!(member.kind, BuildingArchetypeMemberKind::WorldItemPile);
    assert_eq!(member.world_item_state.as_ref().unwrap().stack_quantity, Some(5));
    let serialized = ron::ser::to_string(member).unwrap();
    assert!(!serialized.contains("item_pile_id"));
    assert!(!serialized.contains("ItemPileId"));
    let _ = std::fs::remove_dir_all(dir);
}
