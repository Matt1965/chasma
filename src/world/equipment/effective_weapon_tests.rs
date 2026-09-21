//! Effective equipped-weapon resolution tests (Slice 3).

use bevy::prelude::Vec3;

use crate::world::equipment::{
    effective_weapon_for_unit, effective_weapon_id_for_unit, equipped_armor_for_unit,
};
use crate::world::{
    ArmorProfileCatalog, ArmorProfileId, AttackTargetingPolicy, ChunkCoord, ChunkData, ChunkId,
    ChunkLayout, CombatStrikeEvent, CombatStrikeReport, DamageType, DoodadCatalog, EquipmentSlot,
    Heightfield, HitMode, InventoryCatalogCtx, InventoryProfileCatalog, InventoryProfileId,
    ItemCatalog, ItemCategoryCatalog, LocalPosition, NavigationConfig, TargetFilter,
    TransferPlacementPolicy, UnitCatalog, UnitDefinitionId, UnitOrder, UnitOwnership, UnitRecord,
    UnitSource, WeaponCatalog, WeaponDefinition, WeaponDefinitionId, WeaponTiming, WorldData,
    WorldPosition, create_item_instance, create_unit_with_inventory, create_unit_with_ownership,
    issue_unit_order, place_unique_first_fit, starter_armor_profile_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_unit_definitions, step_all_combat_strikes, test_equipment_fixture_definitions,
    transfer_unique_item, weapon_for_unit_record,
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
    static CATEGORIES: std::sync::OnceLock<ItemCategoryCatalog> = std::sync::OnceLock::new();
    static ITEMS: std::sync::OnceLock<ItemCatalog> = std::sync::OnceLock::new();
    static PROFILES: std::sync::OnceLock<InventoryProfileCatalog> = std::sync::OnceLock::new();

    let categories = CATEGORIES.get_or_init(|| {
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap()
    });
    let items = ITEMS.get_or_init(|| {
        ItemCatalog::from_definitions(test_equipment_fixture_definitions(), categories).unwrap()
    });
    let profiles = PROFILES.get_or_init(|| {
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap()
    });
    InventoryCatalogCtx::new(items, categories, profiles)
}

fn weapons() -> WeaponCatalog {
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

fn bandit(world: &mut WorldData, ctx: &InventoryCatalogCtx<'_>) -> UnitRecord {
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

fn equip_iron_sword(world: &mut WorldData, ctx: &InventoryCatalogCtx<'_>, unit: &UnitRecord) {
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
    let _ = sword_id;
}

fn policy() -> AttackTargetingPolicy {
    AttackTargetingPolicy::default()
}

fn step_strike(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    item_catalog: &ItemCatalog,
    armor_catalog: &ArmorProfileCatalog,
    weapons: &WeaponCatalog,
    delta: f32,
) -> CombatStrikeReport {
    let mut projectile = crate::world::ProjectileReport::default();
    step_all_combat_strikes(
        world,
        unit_catalog,
        weapons,
        item_catalog,
        armor_catalog,
        &DoodadCatalog::default(),
        &NavigationConfig::default(),
        policy(),
        delta,
        &mut projectile,
    )
}

#[test]
fn starter_equipment_items_have_slot_compatibility() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items =
        ItemCatalog::from_definitions(test_equipment_fixture_definitions(), &categories).unwrap();
    let helmet = items
        .get(&crate::world::ItemDefinitionId::new("ranger_hood"))
        .unwrap();
    assert!(helmet.equipment_slots.contains(&EquipmentSlot::Head));
    assert_eq!(
        helmet.armor_profile_id,
        Some(ArmorProfileId::new("armor_ranger_hood"))
    );
    let sword = items
        .get(&crate::world::ItemDefinitionId::new("iron_sword"))
        .unwrap();
    assert!(sword.equipment_slots.contains(&EquipmentSlot::Weapon));
    assert_eq!(
        sword.weapon_definition_id,
        Some(WeaponDefinitionId::new("weapon_iron_sword"))
    );
}

#[test]
fn effective_weapon_falls_back_to_unit_default_without_equipment() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, &ctx);
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapons = weapons();
    let id = effective_weapon_id_for_unit(&world, &unit, &unit_catalog, ctx.items).unwrap();
    assert_eq!(id, WeaponDefinitionId::new("weapon_fists"));
    let weapon =
        effective_weapon_for_unit(&world, &unit, &unit_catalog, ctx.items, &weapons).unwrap();
    assert_eq!(weapon.damage, 4.0);
    assert_eq!(weapon.range_meters, 1.2);
}

#[test]
fn equipped_iron_sword_controls_effective_weapon() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            &ctx,
            crate::world::ItemDefinitionId::new("iron_sword"),
            Default::default(),
        )
        .unwrap()
    };
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, sword_id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            sword_id,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapons = weapons();
    let weapon =
        effective_weapon_for_unit(&world, &unit, &unit_catalog, ctx.items, &weapons).unwrap();
    assert_eq!(weapon.id, WeaponDefinitionId::new("weapon_iron_sword"));
    assert_eq!(weapon.damage, 14.0);
    assert_eq!(weapon.range_meters, 2.2);
}

#[test]
fn unequipped_iron_sword_returns_to_default_weapon() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let weapon_slot = unit.equipment.unwrap().weapon;
    let sword_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let id = create_item_instance(
            inventory_store,
            instance_store,
            &ctx,
            crate::world::ItemDefinitionId::new("iron_sword"),
            Default::default(),
        )
        .unwrap();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            id,
            weapon_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
        id
    };
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            weapon_slot,
            0,
            sword_id,
            personal,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapons = weapons();
    let weapon =
        effective_weapon_for_unit(&world, &unit, &unit_catalog, ctx.items, &weapons).unwrap();
    assert_eq!(weapon.id, WeaponDefinitionId::new("weapon_fists"));
}

#[test]
fn equipped_armor_resolves_profiles_without_mitigation_side_effects() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = bandit(&mut world, &ctx);
    let personal = unit.inventory_id.unwrap();
    let head_slot = unit.equipment.unwrap().head;
    let helmet_id = {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        create_item_instance(
            inventory_store,
            instance_store,
            &ctx,
            crate::world::ItemDefinitionId::new("ranger_hood"),
            Default::default(),
        )
        .unwrap()
    };
    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_unique_first_fit(inventory_store, instance_store, &ctx, personal, helmet_id).unwrap();
        transfer_unique_item(
            inventory_store,
            instance_store,
            &ctx,
            personal,
            0,
            helmet_id,
            head_slot,
            TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
        )
        .unwrap();
    }
    let armor = ArmorProfileCatalog::from_definitions(starter_armor_profile_definitions()).unwrap();
    let entries = equipped_armor_for_unit(&world, &unit, ctx.items, &armor).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].slot, EquipmentSlot::Head);
    assert_eq!(
        entries[0].armor_profile_id,
        ArmorProfileId::new("armor_ranger_hood")
    );
}

#[cfg(all(feature = "data-import", test))]
mod workbook_import_tests {
    use super::*;
    use crate::data_import::dev_design_workbook_path;
    use crate::data_import::{import_armor_profiles_from_excel, import_item_catalog_from_excel};

    #[test]
    fn starter_equipment_imported_from_design_workbook() {
        let path = dev_design_workbook_path();
        assert!(path.exists(), "workbook missing at {}", path.display());
        let (_, items, summary) = import_item_catalog_from_excel(&path).unwrap();
        assert_eq!(
            summary.rows_failed, 0,
            "import warnings: {:?}",
            summary.warnings
        );
        for id in [
            "iron_sword",
            "leather_backpack",
            "ranger_hood",
            "ranger_body",
            "ranger_arms",
            "ranger_legs",
            "ranger_feet",
        ] {
            assert!(
                items
                    .get(&crate::world::ItemDefinitionId::new(id))
                    .is_some(),
                "expected workbook item `{id}`"
            );
        }
        let sword = items
            .get(&crate::world::ItemDefinitionId::new("iron_sword"))
            .unwrap();
        assert!(sword.equipment_slots.contains(&EquipmentSlot::Weapon));
        assert_eq!(
            sword.weapon_definition_id,
            Some(WeaponDefinitionId::new("weapon_iron_sword"))
        );
        let backpack = items
            .get(&crate::world::ItemDefinitionId::new("leather_backpack"))
            .unwrap();
        assert_eq!(
            backpack.backpack_profile_id,
            Some(InventoryProfileId::new("backpack_basic_internal"))
        );
        let (armor_defs, armor_summary) = import_armor_profiles_from_excel(&path).unwrap();
        assert_eq!(armor_summary.rows_failed, 0);
        let armor = ArmorProfileCatalog::from_definitions(armor_defs).unwrap();
        let helmet = items
            .get(&crate::world::ItemDefinitionId::new("ranger_hood"))
            .unwrap();
        let profile_id = helmet.armor_profile_id.clone().unwrap();
        assert!(armor.get(&profile_id).is_some());
    }
}

#[test]
fn leather_backpack_resolves_authored_internal_profile() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items =
        ItemCatalog::from_definitions(test_equipment_fixture_definitions(), &categories).unwrap();
    let backpack = items
        .get(&crate::world::ItemDefinitionId::new("leather_backpack"))
        .unwrap();
    assert_eq!(
        backpack.backpack_profile_id,
        Some(InventoryProfileId::new("backpack_basic_internal"))
    );
}

#[test]
fn combat_damage_uses_equipped_iron_sword() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapons = weapons();
    let attacker_record = {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        create_unit_with_inventory(
            &catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(10.0, 10.0),
            UnitSource::Dev,
            UnitOwnership::player_default(),
            &ctx,
        )
        .unwrap()
    };
    let attacker = attacker_record.id;
    equip_iron_sword(&mut world, &ctx, &attacker_record);
    let hostile = create_unit_with_ownership(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(11.0, 10.0),
        UnitSource::Authored,
        UnitOwnership::hostile(),
    )
    .unwrap()
    .id;
    world
        .mutate_unit(hostile, |record| {
            record.vitals.max_hp = 100;
            record.vitals.current_hp = 100;
        })
        .expect("hostile exists");
    let hp_before = world.get_unit(hostile).unwrap().vitals.current_hp;
    assert_eq!(hp_before, 100);
    issue_unit_order(
        &mut world,
        &unit_catalog,
        &weapons,
        ctx.items,
        &DoodadCatalog::default(),
        &NavigationConfig::default(),
        attacker,
        UnitOrder::Attack { target: hostile },
        policy(),
    )
    .unwrap();
    step_strike(
        &mut world,
        &unit_catalog,
        ctx.items,
        &ArmorProfileCatalog::from_definitions(starter_armor_profile_definitions()).unwrap(),
        &weapons,
        0.1,
    );
    let report = step_strike(
        &mut world,
        &unit_catalog,
        ctx.items,
        &ArmorProfileCatalog::from_definitions(starter_armor_profile_definitions()).unwrap(),
        &weapons,
        0.1,
    );
    assert!(report.traces.iter().any(|trace| {
        matches!(
            trace.event,
            CombatStrikeEvent::AttackStrikeApplied {
                damage: 14.0,
                target_hp_before,
                target_hp_after,
            } if target_hp_before == hp_before
                && target_hp_after == hp_before.saturating_sub(14)
        )
    }));
}

#[test]
fn combat_range_uses_equipped_iron_sword() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapons = weapons();
    let attacker_record = {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        create_unit_with_inventory(
            &catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(10.0, 10.0),
            UnitSource::Dev,
            UnitOwnership::player_default(),
            &ctx,
        )
        .unwrap()
    };
    equip_iron_sword(&mut world, &ctx, &attacker_record);
    let attacker = world.get_unit(attacker_record.id).unwrap();
    let sword =
        weapon_for_unit_record(&world, attacker, &unit_catalog, ctx.items, &weapons).unwrap();
    assert_eq!(sword.range_meters, 2.2);
    let unarmed = bandit(&mut world, &ctx);
    let fists =
        weapon_for_unit_record(&world, &unarmed, &unit_catalog, ctx.items, &weapons).unwrap();
    assert_eq!(fists.range_meters, 1.2);
}

#[test]
fn attack_timing_uses_equipped_iron_sword() {
    let weapons = weapons();
    let fists = weapons
        .get(&WeaponDefinitionId::new("weapon_fists"))
        .unwrap();
    let sword = weapons
        .get(&WeaponDefinitionId::new("weapon_iron_sword"))
        .unwrap();
    let fists_timing = WeaponTiming::from_weapon(fists);
    let sword_timing = WeaponTiming::from_weapon(sword);
    assert!(fists_timing.attack_period_seconds < sword_timing.attack_period_seconds);
}

#[test]
fn two_units_keep_independent_effective_weapons() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let weapons = weapons();
    let armed = bandit(&mut world, &ctx);
    equip_iron_sword(&mut world, &ctx, &armed);
    let unarmed = bandit(&mut world, &ctx);
    let armed_weapon =
        effective_weapon_for_unit(&world, &armed, &unit_catalog, ctx.items, &weapons).unwrap();
    let unarmed_weapon =
        effective_weapon_for_unit(&world, &unarmed, &unit_catalog, ctx.items, &weapons).unwrap();
    assert_eq!(
        armed_weapon.id,
        WeaponDefinitionId::new("weapon_iron_sword")
    );
    assert_eq!(unarmed_weapon.id, WeaponDefinitionId::new("weapon_fists"));
}

#[test]
fn ranger_armor_items_resolve_armor_profile_ids() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items =
        ItemCatalog::from_definitions(test_equipment_fixture_definitions(), &categories).unwrap();
    let armor = ArmorProfileCatalog::from_definitions(starter_armor_profile_definitions()).unwrap();
    for (item_id, profile_id) in [
        ("ranger_hood", "armor_ranger_hood"),
        ("ranger_body", "armor_ranger_body"),
        ("ranger_arms", "armor_ranger_arms"),
        ("ranger_legs", "armor_ranger_legs"),
        ("ranger_feet", "armor_ranger_feet"),
    ] {
        let item = items
            .get(&crate::world::ItemDefinitionId::new(item_id))
            .unwrap();
        let profile = item.armor_profile_id.clone().unwrap();
        assert_eq!(profile, ArmorProfileId::new(profile_id));
        assert!(armor.get(&profile).is_some());
    }
}
