//! Combat armor mitigation integration tests (Slice 4).

use bevy::prelude::Vec3;

use crate::world::combat::apply_attributed_combat_damage;
use crate::world::equipment::ArmorResolveError;
use crate::world::equipment::slot::EquipmentSlot;
use crate::world::{
    ArmorProfileCatalog, AttackTargetingPolicy, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
    DoodadCatalog, Heightfield, InventoryCatalogCtx, ItemDefinitionId, LocalPosition,
    NavigationConfig, TransferPlacementPolicy, UnitCatalog, UnitDefinitionId, UnitId,
    UnitOwnership, UnitRecord, UnitSource, WeaponCatalog, WorldData, WorldPosition,
    create_item_instance, create_unit_with_inventory, create_unit_with_ownership,
    place_unique_first_fit, starter_armor_profile_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_unit_definitions, test_equipment_fixture_definitions, total_armor_rating_for_unit,
    transfer_unique_item,
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

fn test_ctx() -> InventoryCatalogCtx<'static> {
    static CATEGORIES: std::sync::OnceLock<crate::world::ItemCategoryCatalog> =
        std::sync::OnceLock::new();
    static ITEMS: std::sync::OnceLock<crate::world::ItemCatalog> = std::sync::OnceLock::new();
    static PROFILES: std::sync::OnceLock<crate::world::InventoryProfileCatalog> =
        std::sync::OnceLock::new();

    let categories = CATEGORIES.get_or_init(|| {
        crate::world::ItemCategoryCatalog::from_definitions(starter_item_category_definitions())
            .unwrap()
    });
    let items = ITEMS.get_or_init(|| {
        crate::world::ItemCatalog::from_definitions(
            test_equipment_fixture_definitions(),
            categories,
        )
        .unwrap()
    });
    let profiles = PROFILES.get_or_init(|| {
        crate::world::InventoryProfileCatalog::from_definitions(
            starter_inventory_profile_definitions(),
        )
        .unwrap()
    });
    InventoryCatalogCtx::new(items, categories, profiles)
}

fn armor_catalog() -> ArmorProfileCatalog {
    ArmorProfileCatalog::from_definitions(starter_armor_profile_definitions()).unwrap()
}

fn weapons() -> WeaponCatalog {
    use crate::world::{DamageType, HitMode, TargetFilter, WeaponDefinition, WeaponDefinitionId};

    WeaponCatalog::from_definitions(vec![
        WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_fists"),
            "Fists",
            "",
            4.0,
            DamageType::Blunt,
            1.2,
            1.5,
            0.15,
            0.1,
            HitMode::Melee,
            None,
            0.0,
            "attack_fists",
            vec![TargetFilter::Enemies],
            None,
            true,
        ),
        WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_iron_sword"),
            "Scrap Sword",
            "",
            14.0,
            DamageType::Slashing,
            2.2,
            1.0,
            0.2,
            0.15,
            HitMode::Melee,
            None,
            0.0,
            "attack_fists",
            vec![TargetFilter::Enemies],
            None,
            true,
        ),
    ])
    .unwrap()
}

fn policy() -> AttackTargetingPolicy {
    AttackTargetingPolicy::default()
}

fn set_unit_test_hp(world: &mut WorldData, unit_id: UnitId, hp: u32) {
    world
        .mutate_unit(unit_id, |record| {
            record.vitals.max_hp = hp;
            record.vitals.current_hp = hp;
        })
        .expect("unit exists");
}

fn equip_armor_piece(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit: &UnitRecord,
    slot: EquipmentSlot,
    item_id: &str,
) {
    let personal = unit.inventory_id.unwrap();
    let equipment = unit.equipment.unwrap();
    let slot_inventory = equipment.inventory_id(slot);
    let item_id = ItemDefinitionId::new(item_id);
    let instance_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            ctx,
            item_id,
            Default::default(),
        )
        .unwrap()
    };
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, ctx, personal, instance_id)
            .unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            personal,
            0,
            instance_id,
            slot_inventory,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
}

fn equip_starter_armor(world: &mut WorldData, ctx: &InventoryCatalogCtx<'_>, unit: &UnitRecord) {
    let pieces = [
        (EquipmentSlot::Head, "ranger_hood"),
        (EquipmentSlot::Body, "ranger_body"),
        (EquipmentSlot::Arms, "ranger_arms"),
        (EquipmentSlot::Legs, "ranger_legs"),
        (EquipmentSlot::Feet, "ranger_feet"),
    ];
    for (slot, item_id) in pieces {
        equip_armor_piece(world, ctx, unit, slot, item_id);
    }
}

fn unequip_slot(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit: &UnitRecord,
    slot: EquipmentSlot,
) {
    let personal = unit.inventory_id.unwrap();
    let slot_inventory = unit.equipment.unwrap().inventory_id(slot);
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let entry = inventory_store
        .get(slot_inventory)
        .and_then(|record| record.placed_entries().first())
        .expect("equipped entry");
    let instance_id = match &entry.contents {
        crate::world::inventory::InventoryEntryContents::Unique { item_instance_id } => {
            *item_instance_id
        }
        _ => panic!("expected unique equipped item"),
    };
    transfer_unique_item(
        inventory_store,
        instance_store,
        ctx,
        slot_inventory,
        0,
        instance_id,
        personal,
        TransferPlacementPolicy::FirstFitOnly,
    )
    .unwrap();
}

fn apply_combat_hit(
    world: &mut WorldData,
    attacker: UnitId,
    defender: UnitId,
    raw_damage: f32,
    ctx: &InventoryCatalogCtx<'_>,
    armor: &ArmorProfileCatalog,
) -> u32 {
    let hp_before = world.get_unit(defender).unwrap().vitals.current_hp;
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapon_catalog = weapons();
    let vitals = apply_attributed_combat_damage(
        world,
        defender,
        attacker,
        raw_damage,
        &unit_catalog,
        &weapon_catalog,
        ctx.items,
        armor,
        &DoodadCatalog::default(),
        &NavigationConfig::default(),
        policy(),
    )
    .unwrap();
    hp_before.saturating_sub(vitals.current_hp)
}

#[test]
fn no_armor_equipped_totals_zero() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    let rating = total_armor_rating_for_unit(&world, &unit, ctx.items, &armor_catalog()).unwrap();
    assert_eq!(rating, 0);
}

#[test]
fn single_equipped_armor_piece_resolves_rating() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_armor_piece(&mut world, &ctx, &unit, EquipmentSlot::Body, "ranger_body");
    let rating = total_armor_rating_for_unit(&world, &unit, ctx.items, &armor_catalog()).unwrap();
    assert_eq!(rating, 15);
}

#[test]
fn multiple_equipped_pieces_sum_ratings() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_armor_piece(&mut world, &ctx, &unit, EquipmentSlot::Head, "ranger_hood");
    equip_armor_piece(&mut world, &ctx, &unit, EquipmentSlot::Body, "ranger_body");
    let rating = total_armor_rating_for_unit(&world, &unit, ctx.items, &armor_catalog()).unwrap();
    assert_eq!(rating, 20);
}

#[test]
fn full_starter_armor_totals_forty_rating() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_starter_armor(&mut world, &ctx, &unit);
    let armor = armor_catalog();
    let rating = total_armor_rating_for_unit(&world, &unit, ctx.items, &armor).unwrap();
    assert_eq!(rating, 40);
}

#[test]
fn equip_and_unequip_update_total_armor_immediately() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    let armor = armor_catalog();
    assert_eq!(
        total_armor_rating_for_unit(&world, &unit, ctx.items, &armor).unwrap(),
        0
    );
    equip_armor_piece(&mut world, &ctx, &unit, EquipmentSlot::Body, "ranger_body");
    assert_eq!(
        total_armor_rating_for_unit(&world, &unit, ctx.items, &armor).unwrap(),
        15
    );
    equip_armor_piece(&mut world, &ctx, &unit, EquipmentSlot::Head, "ranger_hood");
    assert_eq!(
        total_armor_rating_for_unit(&world, &unit, ctx.items, &armor).unwrap(),
        20
    );
    unequip_slot(&mut world, &ctx, &unit, EquipmentSlot::Body);
    assert_eq!(
        total_armor_rating_for_unit(&world, &unit, ctx.items, &armor).unwrap(),
        5
    );
}

#[test]
fn iron_sword_damage_mitigated_to_ten_in_combat() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let armor = armor_catalog();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapon_catalog = weapons();

    let attacker = create_unit_with_ownership(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(10.0, 10.0),
        UnitSource::Dev,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;

    let defender_record = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(11.0, 10.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_starter_armor(&mut world, &ctx, &defender_record);
    let defender = defender_record.id;
    set_unit_test_hp(&mut world, defender, 100);
    let hp_before = world.get_unit(defender).unwrap().vitals.current_hp;
    let rating =
        total_armor_rating_for_unit(&world, world.get_unit(defender).unwrap(), ctx.items, &armor)
            .unwrap();
    assert_eq!(rating, 40);

    let iron_sword = weapon_catalog
        .get(&crate::world::WeaponDefinitionId::new("weapon_iron_sword"))
        .unwrap();
    assert_eq!(iron_sword.damage, 14.0);

    let expected_applied = crate::world::resolve_applied_combat_damage(iron_sword.damage, rating);
    assert_eq!(expected_applied, 10);

    let vitals = apply_attributed_combat_damage(
        &mut world,
        defender,
        attacker,
        iron_sword.damage,
        &unit_catalog,
        &weapon_catalog,
        ctx.items,
        &armor,
        &DoodadCatalog::default(),
        &NavigationConfig::default(),
        policy(),
    )
    .unwrap();

    assert_eq!(
        vitals.current_hp,
        hp_before.saturating_sub(expected_applied)
    );
}

#[test]
fn combat_hit_without_armor_deals_full_damage() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let armor = armor_catalog();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let attacker = create_unit_with_ownership(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let defender = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(2.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap()
    .id;
    set_unit_test_hp(&mut world, defender, 100);
    let applied = apply_combat_hit(&mut world, attacker, defender, 14.0, &ctx, &armor);
    assert_eq!(applied, 14);
}

#[test]
fn removing_armor_restores_unmitigated_damage() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let armor = armor_catalog();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let attacker = create_unit_with_ownership(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let defender_record = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(2.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_starter_armor(&mut world, &ctx, &defender_record);
    let defender = defender_record.id;
    set_unit_test_hp(&mut world, defender, 100);
    let mitigated = apply_combat_hit(&mut world, attacker, defender, 14.0, &ctx, &armor);
    assert_eq!(mitigated, 10);
    set_unit_test_hp(&mut world, defender, 100);
    for slot in [
        EquipmentSlot::Head,
        EquipmentSlot::Body,
        EquipmentSlot::Arms,
        EquipmentSlot::Legs,
        EquipmentSlot::Feet,
    ] {
        unequip_slot(&mut world, &ctx, &defender_record, slot);
    }
    assert_eq!(
        total_armor_rating_for_unit(&world, world.get_unit(defender).unwrap(), ctx.items, &armor,)
            .unwrap(),
        0
    );
    let unmitigated = apply_combat_hit(&mut world, attacker, defender, 14.0, &ctx, &armor);
    assert_eq!(unmitigated, 14);
}

#[test]
fn projectile_and_melee_share_attributed_combat_damage_seam() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let armor = armor_catalog();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let attacker = create_unit_with_ownership(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let defender_record = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(2.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_starter_armor(&mut world, &ctx, &defender_record);
    let defender = defender_record.id;
    set_unit_test_hp(&mut world, defender, 100);

    // Melee strike and projectile impact both call `apply_attributed_combat_damage`.
    let melee_applied = apply_combat_hit(&mut world, attacker, defender, 14.0, &ctx, &armor);
    set_unit_test_hp(&mut world, defender, 100);
    let projectile_applied = apply_combat_hit(&mut world, attacker, defender, 14.0, &ctx, &armor);
    assert_eq!(melee_applied, 10);
    assert_eq!(projectile_applied, 10);
}

#[test]
fn direct_damage_unit_bypasses_armor() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let defender_record = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_starter_armor(&mut world, &ctx, &defender_record);
    let defender = defender_record.id;
    set_unit_test_hp(&mut world, defender, 100);
    let hp_before = world.get_unit(defender).unwrap().vitals.current_hp;
    world.damage_unit(defender, 14).unwrap();
    let hp_after = world.get_unit(defender).unwrap().vitals.current_hp;
    assert_eq!(hp_after, hp_before - 14);
}

#[test]
fn two_units_resolve_armor_independently() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let armor = armor_catalog();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let lightly_armored = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_armor_piece(
        &mut world,
        &ctx,
        &lightly_armored,
        EquipmentSlot::Head,
        "ranger_hood",
    );
    let fully_armored = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(2.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_starter_armor(&mut world, &ctx, &fully_armored);
    assert_eq!(
        total_armor_rating_for_unit(&world, &lightly_armored, ctx.items, &armor).unwrap(),
        5
    );
    assert_eq!(
        total_armor_rating_for_unit(&world, &fully_armored, ctx.items, &armor).unwrap(),
        40
    );
}

#[test]
fn broken_armor_profile_link_errors_instead_of_zero() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let armor = ArmorProfileCatalog::from_definitions(vec![]).unwrap();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Dev,
        UnitOwnership::hostile(),
        &ctx,
    )
    .unwrap();
    equip_armor_piece(&mut world, &ctx, &unit, EquipmentSlot::Head, "ranger_hood");
    let err = total_armor_rating_for_unit(&world, &unit, ctx.items, &armor).unwrap_err();
    assert!(matches!(err, ArmorResolveError::MissingArmorProfile { .. }));
}
