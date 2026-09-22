use std::sync::OnceLock;

use bevy::prelude::{Quat, Vec3};

use crate::world::inventory::{
    InventoryCatalogCtx, InventoryOwnerRef, ItemInstanceMetadata, capture_inventory_subgraph,
    create_inventory, create_item_instance, inventory_subgraph_item_count, place_stack_first_fit,
    place_unique_first_fit,
};
use crate::world::item_pile::{
    DropReport, ItemPileSettings, ItemPileTransformCandidate, PileOwnership,
    drop_stack_from_inventory, update_item_pile_transform,
};
use crate::world::{
    Affiliation, BuildingArchetypeId, BuildingArchetypeReconstructCtx, BuildingCategoryCatalog,
    BuildingDefinitionId, BuildingLifecycleState, BuildingOwnership, BuildingSource, ChunkCoord,
    ChunkData, ChunkExtent, ChunkId, DoodadCatalog, DoodadSource, FootprintCatalog, Heightfield,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, LocalPosition,
    OperationCatalog, SpaceId, WorldData, WorldPosition, apply_building_archetype_placement,
    build_building_archetype_definition, capture_building_archetype_members,
    create_building, create_building_with_inventory, create_dev_complete_building,
    create_doodad, inventory_subgraph_item_count as count_items, DoodadDefinitionId,
    DoodadPlacementOverrides,
};

use super::building::{
    BuildingArchetypeLocalPose, BuildingArchetypeMember, BuildingArchetypeMemberKind,
};

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
            ItemCategoryCatalog::from_definitions(crate::world::starter_item_category_definitions())
                .unwrap();
        let mut items = crate::world::starter_item_definitions();
        items.extend(crate::world::test_equipment_fixture_definitions());
        let items = ItemCatalog::from_definitions(items, &categories).unwrap();
        let profiles = InventoryProfileCatalog::from_definitions(
            crate::world::starter_inventory_profile_definitions(),
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
    crate::world::BuildingCatalog::from_definitions(
        crate::world::starter_building_definitions(),
        &categories,
    )
    .unwrap()
}

fn reconstruct_ctx<'a>(
    building_catalog: &'a crate::world::BuildingCatalog,
    doodad_catalog: &'a DoodadCatalog,
    item_catalog: &'a ItemCatalog,
    footprint_catalog: &'a FootprintCatalog,
    operation_catalog: &'a OperationCatalog,
) -> BuildingArchetypeReconstructCtx<'a> {
    static INTERIOR: OnceLock<crate::world::InteriorProfileCatalog> = OnceLock::new();
    let interior_catalog = INTERIOR.get_or_init(crate::world::InteriorProfileCatalog::default);
    BuildingArchetypeReconstructCtx {
        building_catalog,
        doodad_catalog,
        item_catalog,
        operation_catalog,
        interior_catalog,
        inventory_ctx: test_inventory_ctx(),
        occupancy: crate::world::OccupancyCatalogs {
            doodad: doodad_catalog,
            building: building_catalog,
            footprint: footprint_catalog,
        },
        nav_catalog: None,
        created_tick: 1,
    }
}

fn capture_shop(world: &WorldData, root: &crate::world::BuildingRecord) -> crate::world::BuildingArchetypeDefinition {
    let (metadata, members) = capture_building_archetype_members(
        world,
        root,
        &building_catalog(),
        &FootprintCatalog::default(),
        &DoodadCatalog::default(),
        3.0,
    )
    .unwrap();
    build_building_archetype_definition(
        BuildingArchetypeId::new("test_shop"),
        "Test Shop".to_string(),
        root,
        world,
        metadata,
        members,
        true,
    )
}

fn spawn_root(world: &mut WorldData, x: f32, z: f32) -> crate::world::BuildingRecord {
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
    });
    created
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

fn place_archetype(
    world: &mut WorldData,
    archetype: &crate::world::BuildingArchetypeDefinition,
    x: f32,
    z: f32,
    yaw_deg: f32,
) -> crate::world::BuildingRecord {
    let catalog = building_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    let operation_catalog = OperationCatalog::default();
    let item_catalog = test_inventory_ctx().items;
    let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
    let root = create_dev_complete_building(
        &catalog,
        world,
        &archetype.base_building_id,
        pos(x, z),
        rotation,
        BuildingOwnership {
            owner_id: archetype.snapshot.owner_id,
            team_id: archetype.snapshot.team_id,
            affiliation: archetype.snapshot.affiliation,
        },
        None,
    )
    .unwrap();
    let ctx = reconstruct_ctx(
        &catalog,
        &doodad_catalog,
        item_catalog,
        &footprint_catalog,
        &operation_catalog,
    );
    apply_building_archetype_placement(world, root.id, archetype, &ctx).unwrap();
    world.get_building(root.id).unwrap().clone()
}

#[test]
fn reconstructs_building_member_and_world_item() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    let _chest = spawn_chest(&mut world, 52.0, 50.0);
    drop_stack_at(&mut world, 53.0, 50.0, 3);
    let archetype = capture_shop(&world, &root);

    let mut placed = layout_world();
    let placed_root = place_archetype(&mut placed, &archetype, 120.0, 120.0, 0.0);
    assert_eq!(
        placed
            .buildings_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .map(|store| store.len())
            .unwrap_or(0),
        2
    );
    let piles = placed
        .item_pile_store()
        .piles_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)));
    assert_eq!(piles.len(), 1);
    assert_eq!(piles[0].stack_quantity(), Some(3));
}

#[test]
fn reconstructs_world_item_yaw_relative_to_root() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    let report = drop_stack_at(&mut world, 53.0, 50.0, 1);
    let pile_id = report.created_pile_ids[0];
    let placement = world.item_pile_store().get(pile_id).unwrap().placement;
    update_item_pile_transform(
        &mut world,
        pile_id,
        ItemPileTransformCandidate {
            position: placement,
            yaw_degrees: 30.0,
        },
    )
    .unwrap();
    let archetype = capture_shop(&world, &root);

    let mut placed = layout_world();
    let _placed_root = place_archetype(&mut placed, &archetype, 120.0, 120.0, 90.0);
    let pile = placed
        .item_pile_store()
        .piles_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
        .into_iter()
        .next()
        .expect("reconstructed pile");
    assert!(
        (pile.yaw_degrees - 120.0).abs() < 0.5,
        "expected ~120° world yaw, got {}",
        pile.yaw_degrees
    );
}

#[test]
fn legacy_world_item_member_without_local_yaw_defaults_relative_zero() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    drop_stack_at(&mut world, 53.0, 50.0, 1);
    let archetype = capture_shop(&world, &root);
    let member = archetype
        .members
        .iter()
        .find(|member| member.kind == BuildingArchetypeMemberKind::WorldItemPile)
        .expect("pile member");
    let legacy = BuildingArchetypeMember {
        kind: member.kind,
        definition_id: member.definition_id.clone(),
        local_pose: BuildingArchetypeLocalPose {
            local_position: member.local_pose.local_position,
            local_rotation: [0.0, 0.0, 0.0, 1.0],
            uniform_scale_milli: member.local_pose.uniform_scale_milli,
            scale_x_milli: member.local_pose.scale_x_milli,
            scale_y_milli: member.local_pose.scale_y_milli,
            scale_z_milli: member.local_pose.scale_z_milli,
        },
        building_state: member.building_state.clone(),
        world_item_state: member.world_item_state.clone(),
    };
    let legacy_archetype = crate::world::BuildingArchetypeDefinition {
        id: archetype.id.clone(),
        display_name: archetype.display_name.clone(),
        base_building_id: archetype.base_building_id.clone(),
        snapshot: archetype.snapshot.clone(),
        capture_metadata: archetype.capture_metadata.clone(),
        members: vec![legacy],
        enabled: archetype.enabled,
    };

    let mut placed = layout_world();
    let _placed_root = place_archetype(&mut placed, &legacy_archetype, 120.0, 120.0, 45.0);
    let pile = placed
        .item_pile_store()
        .piles_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
        .into_iter()
        .next()
        .expect("reconstructed pile");
    assert!(
        (pile.yaw_degrees - 45.0).abs() < 0.5,
        "legacy relative yaw 0 should match root yaw, got {}",
        pile.yaw_degrees
    );
}

#[test]
fn reconstructs_nested_member_inventory() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
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
            5,
        )
        .unwrap();
        let backpack = create_item_instance(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            ItemDefinitionId::new("leather_backpack"),
            ItemInstanceMetadata::default(),
        )
        .unwrap();
        place_unique_first_fit(
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
            .expect("internal");
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
    let archetype = capture_shop(&world, &root);
    let member = archetype
        .members
        .iter()
        .find(|member| member.kind == BuildingArchetypeMemberKind::Building)
        .expect("chest member");
    let snapshot = member
        .building_state
        .as_ref()
        .and_then(|state| state.extensions.inventory.as_ref())
        .expect("inventory snapshot");
    assert_eq!(count_items(snapshot), 3);

    let mut placed = layout_world();
    place_archetype(&mut placed, &archetype, 80.0, 80.0, 0.0);
    let placed_chest = placed
        .buildings_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
        .expect("chunk buildings")
        .records()
        .iter()
        .find(|building| building.definition_id.as_str() == "storage_chest")
        .expect("placed chest");
    let restored = capture_inventory_subgraph(
        &placed,
        placed_chest.inventory_id.expect("inventory"),
    )
    .expect("restored inventory");
    assert_eq!(inventory_subgraph_item_count(&restored), 3);
}

#[test]
fn two_spawns_have_independent_inventories() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    let chest = spawn_chest(&mut world, 52.0, 50.0);
    let inventory_id = chest.inventory_id.expect("inventory");
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
    let archetype = capture_shop(&world, &root);

    let mut placed = layout_world();
    place_archetype(&mut placed, &archetype, 80.0, 80.0, 0.0);
    place_archetype(&mut placed, &archetype, 140.0, 140.0, 0.0);

    let chests = placed
        .buildings_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
        .expect("chunk buildings")
        .records()
        .iter()
        .filter(|building| building.definition_id.as_str() == "storage_chest")
        .collect::<Vec<_>>();
    assert_eq!(chests.len(), 2);
    let first_inventory = chests[0].inventory_id.expect("first");
    let second_inventory = chests[1].inventory_id.expect("second");
    assert_ne!(first_inventory, second_inventory);

    let (inventory_store, instance_store) = placed.inventory_runtime_mut();
    crate::world::inventory::remove_entry(
        inventory_store,
        instance_store,
        test_inventory_ctx(),
        first_inventory,
        0,
    )
    .unwrap();
    let second_count = placed
        .inventory_store()
        .get(second_inventory)
        .map(|record| record.placed_entries().len())
        .unwrap_or(0);
    assert_eq!(second_count, 1);
}

#[test]
fn rotated_root_preserves_member_offset() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    drop_stack_at(&mut world, 52.0, 50.0, 1);
    let archetype = capture_shop(&world, &root);

    let mut placed = layout_world();
    place_archetype(&mut placed, &archetype, 100.0, 100.0, 90.0);
    let pile = placed
        .item_pile_store()
        .piles_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))[0]
        .clone();
    let placed_root_id = placed
        .buildings_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
        .expect("chunk buildings")
        .records()
        .iter()
        .find(|building| building.definition_id.as_str() == "hut")
        .expect("root")
        .id;
    let root_record = placed.get_building(placed_root_id).unwrap();
    let root_global = root_record.placement.position.to_global(placed.layout());
    let pile_global = pile.placement.to_global(placed.layout());
    let offset = pile_global - root_global;
    let expected = root_record.placement.rotation * Vec3::new(2.0, 0.0, 0.0);
    assert!((offset.x - expected.x).abs() < 0.2);
    assert!((offset.z - expected.z).abs() < 0.2);
}

#[test]
fn inventory_items_not_duplicated_as_world_piles() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    let chest = spawn_chest(&mut world, 52.0, 50.0);
    let inventory_id = chest.inventory_id.expect("inventory");
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            inventory_id,
            ItemDefinitionId::new("bread"),
            2,
        )
        .unwrap();
    }
    let archetype = capture_shop(&world, &root);

    let mut placed = layout_world();
    place_archetype(&mut placed, &archetype, 90.0, 90.0, 0.0);
    let piles = placed
        .item_pile_store()
        .piles_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)));
    assert!(piles.is_empty());
}

#[test]
fn reconstructs_doodad_member() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    let _tree = create_doodad(
        &DoodadCatalog::default(),
        &mut world,
        &DoodadDefinitionId::new("tree_oak"),
        pos(52.0, 50.0),
        DoodadSource::Dev,
        DoodadPlacementOverrides::default(),
        None,
    )
    .unwrap();
    let archetype = capture_shop(&world, &root);
    assert!(archetype
        .members
        .iter()
        .any(|member| member.kind == BuildingArchetypeMemberKind::Doodad));

    let mut placed = layout_world();
    place_archetype(&mut placed, &archetype, 70.0, 70.0, 0.0);
    let tree_count = placed
        .doodads_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
        .expect("doodads")
        .records()
        .iter()
        .filter(|doodad| doodad.definition_id.as_str() == "tree_oak")
        .count();
    assert_eq!(tree_count, 1);
}

#[test]
fn invalid_member_definition_fails_without_partial_spawn() {
    let mut world = layout_world();
    let root = spawn_root(&mut world, 50.0, 50.0);
    let archetype = capture_shop(&world, &root);
    let mut broken = archetype.clone();
    broken.members.push(crate::world::BuildingArchetypeMember {
        kind: BuildingArchetypeMemberKind::Doodad,
        definition_id: "missing_doodad".to_string(),
        local_pose: crate::world::BuildingArchetypeLocalPose {
            local_position: [1.0, 0.0, 0.0],
            local_rotation: [0.0, 0.0, 0.0, 1.0],
            uniform_scale_milli: 0,
            scale_x_milli: 1000,
            scale_y_milli: 1000,
            scale_z_milli: 1000,
        },
        building_state: None,
        world_item_state: None,
    });

    let catalog = building_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    let operation_catalog = OperationCatalog::default();
    let item_catalog = test_inventory_ctx().items;
    let placed_root = create_dev_complete_building(
        &catalog,
        &mut world,
        &broken.base_building_id,
        pos(200.0, 200.0),
        Quat::IDENTITY,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let ctx = reconstruct_ctx(
        &catalog,
        &doodad_catalog,
        item_catalog,
        &footprint_catalog,
        &operation_catalog,
    );
    assert!(apply_building_archetype_placement(&mut world, placed_root.id, &broken, &ctx).is_err());
    assert!(
        world
            .doodads_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .is_none_or(|store| store.is_empty())
    );
}
