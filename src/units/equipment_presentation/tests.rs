//! Equipment presentation reconciliation tests (Slice 7).

use bevy::prelude::{Quat, Vec3};

use crate::world::equipment::{
    EquipmentAttachmentSocket, EquipmentPresentationAuthoring, EquipmentPresentationMode,
    EquipmentSlot, EquipmentVisualCatalog, EquipmentVisualMapping,
};
use crate::world::{
    ArmorProfileId, ChunkCoord, ChunkData, ChunkId, ChunkLayout, EntryIndex, Heightfield,
    InventoryCatalogCtx, InventoryEntryContents, InventoryId, InventoryProfileCatalog,
    InventoryProfileId, ItemCatalog, ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId,
    ItemDefinition, ItemDefinitionId, ItemInstanceId, ItemInstanceMetadata, ItemRenderKey,
    LocalPosition, TransferPlacementPolicy, UnitCatalog, UnitDefinitionId, UnitOwnership,
    UnitSource, WorldData, WorldPosition, create_item_instance, create_unit_with_inventory,
    place_unique_first_fit, starter_inventory_profile_definitions, starter_unit_definitions,
    transfer_unique_item,
};

use super::profile::bone_suffix_for_socket;
use super::resolve::{
    DesiredEquipmentPresentation, EquipmentPresentationKey,
    desired_equipment_presentations_for_unit,
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

const TEST_UNIT_RENDER_KEY: &str = "bandit";

fn test_items() -> &'static ItemCatalog {
    test_ctx().items
}

fn test_visuals() -> &'static EquipmentVisualCatalog {
    static VISUALS: std::sync::OnceLock<EquipmentVisualCatalog> = std::sync::OnceLock::new();
    VISUALS.get_or_init(|| {
        EquipmentVisualCatalog::from_mappings(vec![
            visual_mapping(
                "test_sword",
                "test_sword",
                EquipmentPresentationMode::RigidAttachment,
                Some(EquipmentAttachmentSocket::RightHand),
            ),
            visual_mapping(
                "test_helmet",
                "test_helmet",
                EquipmentPresentationMode::RigidAttachment,
                Some(EquipmentAttachmentSocket::Head),
            ),
            visual_mapping(
                "test_backpack",
                "test_backpack",
                EquipmentPresentationMode::RigidAttachment,
                Some(EquipmentAttachmentSocket::Back),
            ),
            visual_mapping(
                "ranger_body",
                "equipment/human_male/ranger_body",
                EquipmentPresentationMode::SkinnedOverlay,
                None,
            ),
        ])
        .unwrap()
    })
}

fn visual_mapping(
    item_id: &str,
    equipped_render_key: &str,
    mode: EquipmentPresentationMode,
    socket: Option<EquipmentAttachmentSocket>,
) -> EquipmentVisualMapping {
    EquipmentVisualMapping {
        item_id: ItemDefinitionId::new(item_id),
        unit_render_key: TEST_UNIT_RENDER_KEY.to_string(),
        equipped_render_key: ItemRenderKey::reserved(equipped_render_key),
        mode,
        socket,
        local_translation: Vec3::ZERO,
        local_rotation: Quat::IDENTITY,
        local_scale: Vec3::ONE,
        stowed_socket: None,
        stowed_local_translation: Vec3::ZERO,
        stowed_local_rotation: Quat::IDENTITY,
        stowed_local_scale: Vec3::ONE,
        consumed_morph_params: Vec::new(),
    }
}

fn test_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories = Box::leak(Box::new(
            ItemCategoryCatalog::from_definitions(vec![
                ItemCategoryDefinition::new(ItemCategoryId::new("weapon"), "Weapon", "", true),
                ItemCategoryDefinition::new(ItemCategoryId::new("armor"), "Armor", "", true),
                ItemCategoryDefinition::new(
                    ItemCategoryId::new("container"),
                    "Container",
                    "",
                    true,
                ),
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
                        1_000,
                        1,
                        true,
                    )
                    .with_unique_instance_required(true)
                    .with_equipment_slots(vec![EquipmentSlot::Weapon])
                    .with_weapon_definition_id(crate::world::WeaponDefinitionId::new(
                        "weapon_test_sword",
                    ))
                    .with_render_key(ItemRenderKey::reserved("test_sword")),
                    ItemDefinition::new(
                        ItemDefinitionId::new("test_helmet"),
                        "Test Helmet",
                        "",
                        ItemCategoryId::new("armor"),
                        2,
                        2,
                        false,
                        1,
                        800,
                        1,
                        true,
                    )
                    .with_unique_instance_required(true)
                    .with_equipment_slots(vec![EquipmentSlot::Head])
                    .with_armor_profile_id(ArmorProfileId::new("armor_test_head"))
                    .with_render_key(ItemRenderKey::reserved("test_helmet")),
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
                    .with_backpack_profile_id(InventoryProfileId::new("backpack_basic_internal"))
                    .with_render_key(ItemRenderKey::reserved("test_backpack")),
                    ItemDefinition::new(
                        ItemDefinitionId::new("no_visual_sword"),
                        "No Visual Sword",
                        "",
                        ItemCategoryId::new("weapon"),
                        1,
                        3,
                        false,
                        1,
                        1_000,
                        1,
                        true,
                    )
                    .with_unique_instance_required(true)
                    .with_equipment_slots(vec![EquipmentSlot::Weapon])
                    .with_weapon_definition_id(
                        crate::world::WeaponDefinitionId::new("weapon_test_sword"),
                    ),
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
        UnitOwnership::hostile(),
        ctx,
    )
    .unwrap()
}

fn equip_unique(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit: &crate::world::UnitRecord,
    item_id: &str,
    slot: EquipmentSlot,
) -> crate::world::ItemInstanceId {
    let personal = unit.inventory_id.unwrap();
    let slot_inventory = unit.equipment.unwrap().inventory_id(slot);
    let instance = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            ItemDefinitionId::new(item_id),
            ItemInstanceMetadata::default(),
        )
        .unwrap()
    };
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_unique_first_fit(inventory_store, instance_store, ctx, personal, instance).unwrap();
    transfer_unique_item(
        inventory_store,
        instance_store,
        ctx,
        personal,
        0,
        instance,
        slot_inventory,
        TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
    )
    .unwrap();
    instance
}

fn entry_index_for_unique(
    world: &WorldData,
    inventory_id: InventoryId,
    item_instance_id: ItemInstanceId,
) -> EntryIndex {
    let inventory = world.inventory_store().get(inventory_id).unwrap();
    inventory
        .placed_entries()
        .iter()
        .position(|entry| {
            matches!(
                &entry.contents,
                InventoryEntryContents::Unique { item_instance_id: id } if *id == item_instance_id
            )
        })
        .expect("unique item entry in inventory")
}

#[test]
fn empty_equipment_has_no_desired_presentations() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let desired = desired_equipment_presentations_for_unit(
        &world,
        test_items(),
        test_visuals(),
        TEST_UNIT_RENDER_KEY,
        &unit,
    );
    assert!(desired.is_empty());
}

#[test]
fn equip_weapon_desires_instance_presentation() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let instance = equip_unique(&mut world, ctx, &unit, "test_sword", EquipmentSlot::Weapon);
    let unit = world.get_unit(unit.id).unwrap();
    let desired = desired_equipment_presentations_for_unit(
        &world,
        test_items(),
        test_visuals(),
        TEST_UNIT_RENDER_KEY,
        unit,
    );
    assert_eq!(desired.len(), 1);
    assert_eq!(desired[0].item_instance_id, instance);
    assert_eq!(desired[0].slot, EquipmentSlot::Weapon);
    assert_eq!(
        desired[0].presentation.socket,
        EquipmentAttachmentSocket::RightHand
    );
}

#[test]
fn unequip_weapon_removes_desired_presentation() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let instance = equip_unique(&mut world, ctx, &unit, "test_sword", EquipmentSlot::Weapon);
    let unit = world.get_unit(unit.id).unwrap().clone();
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let entry = entry_index_for_unique(&world, weapon_slot, instance);
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            weapon_slot,
            entry,
            instance,
            personal,
            TransferPlacementPolicy::MergeThenFirstFit,
        )
        .unwrap();
    }
    let unit = world.get_unit(unit.id).unwrap();
    let desired = desired_equipment_presentations_for_unit(
        &world,
        test_items(),
        test_visuals(),
        TEST_UNIT_RENDER_KEY,
        unit,
    );
    assert!(desired.is_empty());
}

#[test]
fn head_and_backpack_presentations_are_independent() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let helmet = equip_unique(&mut world, ctx, &unit, "test_helmet", EquipmentSlot::Head);
    let unit_id = unit.id;
    let backpack = equip_unique(
        &mut world,
        ctx,
        &unit,
        "test_backpack",
        EquipmentSlot::Backpack,
    );
    let unit = world.get_unit(unit_id).unwrap();
    let desired = desired_equipment_presentations_for_unit(
        &world,
        test_items(),
        test_visuals(),
        TEST_UNIT_RENDER_KEY,
        unit,
    );
    assert_eq!(desired.len(), 2);
    assert!(
        desired
            .iter()
            .any(|row| { row.slot == EquipmentSlot::Head && row.item_instance_id == helmet })
    );
    assert!(
        desired
            .iter()
            .any(|row| { row.slot == EquipmentSlot::Backpack && row.item_instance_id == backpack })
    );
}

#[test]
fn replace_weapon_changes_desired_instance() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    let first = equip_unique(&mut world, ctx, &unit, "test_sword", EquipmentSlot::Weapon);
    let unit = world.get_unit(unit.id).unwrap().clone();
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let second = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            ItemDefinitionId::new("test_sword"),
            ItemInstanceMetadata::default(),
        )
        .unwrap()
    };
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            weapon_slot,
            0,
            first,
            personal,
            TransferPlacementPolicy::MergeThenFirstFit,
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, second).unwrap();
    }
    let second_entry = entry_index_for_unique(&world, personal, second);
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            second_entry,
            second,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    let unit = world.get_unit(unit.id).unwrap();
    let desired = desired_equipment_presentations_for_unit(
        &world,
        test_items(),
        test_visuals(),
        TEST_UNIT_RENDER_KEY,
        unit,
    );
    assert_eq!(desired.len(), 1);
    assert_eq!(desired[0].item_instance_id, second);
    assert_ne!(first, second);
}

#[test]
fn missing_visual_mapping_yields_no_desired_row() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = spawn_bandit(&mut world, ctx);
    equip_unique(
        &mut world,
        ctx,
        &unit,
        "no_visual_sword",
        EquipmentSlot::Weapon,
    );
    let unit = world.get_unit(unit.id).unwrap();
    let desired = desired_equipment_presentations_for_unit(
        &world,
        test_items(),
        test_visuals(),
        TEST_UNIT_RENDER_KEY,
        unit,
    );
    assert!(
        desired.is_empty(),
        "no visual mapping means no desired presentation"
    );
}

#[test]
fn presentation_key_tracks_item_instance_not_definition() {
    let desired = DesiredEquipmentPresentation {
        unit_id: crate::world::UnitId::new(1),
        slot: EquipmentSlot::Weapon,
        item_instance_id: crate::world::ItemInstanceId::new(42),
        item_definition_id: ItemDefinitionId::new("test_sword"),
        unit_render_key: TEST_UNIT_RENDER_KEY.to_string(),
        render_key: ItemRenderKey::reserved("test_sword"),
        mode: EquipmentPresentationMode::RigidAttachment,
        presentation: EquipmentPresentationAuthoring::with_socket(
            EquipmentAttachmentSocket::RightHand,
        ),
    };
    let key = EquipmentPresentationKey::from_desired(&desired);
    assert_eq!(key.item_instance_id, crate::world::ItemInstanceId::new(42));
}

#[test]
fn human_socket_map_resolves_semantic_targets() {
    assert_eq!(
        bone_suffix_for_socket("human_male", EquipmentAttachmentSocket::RightHand),
        Some("hand_r")
    );
    assert_eq!(
        bone_suffix_for_socket("human_female", EquipmentAttachmentSocket::Head),
        Some("Head")
    );
    assert_eq!(
        bone_suffix_for_socket("human_male", EquipmentAttachmentSocket::Back),
        Some("spine_02")
    );
    assert!(bone_suffix_for_socket("unknown_species", EquipmentAttachmentSocket::Head).is_none());
}
