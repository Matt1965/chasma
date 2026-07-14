//! Unit inventory attachment and weight queries (ADR-089 I3).

use crate::world::WorldData;
use crate::world::corpse::CorpseId;
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryError, InventoryId, InventoryStore, ItemInstanceStore,
    create_unit_inventory, query_inventory_weight, transfer_inventory_owner,
};
use crate::world::inventory::{
    InventoryOwnerRef, RemovedInventoryContents, remove_owned_inventory,
};
use crate::world::unit::{UnitCatalog, UnitDefinition, UnitId, UnitRecord};

pub fn attach_inventory_on_unit_create(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit: &mut UnitRecord,
    definition: &UnitDefinition,
) -> Result<(), InventoryError> {
    let Some(profile_id) = definition.inventory_profile_id.clone() else {
        unit.inventory_id = None;
        return Ok(());
    };
    let inventory_id =
        create_unit_inventory(world.inventory_store_mut(), ctx, profile_id, unit.id)?;
    unit.inventory_id = Some(inventory_id);
    Ok(())
}

pub fn unit_inventory_weight_grams(world: &WorldData, unit_id: UnitId) -> u64 {
    let Some(record) = world.get_unit(unit_id) else {
        return 0;
    };
    let Some(inventory_id) = record.inventory_id else {
        return 0;
    };
    world
        .inventory_store()
        .get(inventory_id)
        .map(|inventory| inventory.total_mass_grams())
        .unwrap_or(0)
}

pub fn unit_reference_weight_grams(
    world: &WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
) -> Option<u32> {
    let record = world.get_unit(unit_id)?;
    let inventory_id = record.inventory_id?;
    let inventory = world.inventory_store().get(inventory_id)?;
    let profile = ctx.require_profile(inventory.profile_id()).ok()?;
    profile.reference_weight_grams
}

pub fn unit_over_reference_weight_grams(
    world: &WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
) -> u64 {
    let total = unit_inventory_weight_grams(world, unit_id);
    let reference = unit_reference_weight_grams(world, ctx, unit_id)
        .map(u64::from)
        .unwrap_or(0);
    total.saturating_sub(reference)
}

pub fn unit_encumbrance_ratio(
    world: &WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
) -> Option<f64> {
    let reference = unit_reference_weight_grams(world, ctx, unit_id)?;
    if reference == 0 {
        return None;
    }
    Some(unit_inventory_weight_grams(world, unit_id) as f64 / f64::from(reference))
}

pub fn validate_unit_inventory_owner(
    world: &WorldData,
    unit_id: UnitId,
) -> Result<(), InventoryError> {
    let Some(record) = world.get_unit(unit_id) else {
        return Ok(());
    };
    let Some(inventory_id) = record.inventory_id else {
        return Ok(());
    };
    let inventory = world
        .inventory_store()
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    match inventory.owner() {
        InventoryOwnerRef::Unit(owner) if owner == &unit_id => Ok(()),
        _ => Err(InventoryError::UnitInventoryOwnerMismatch {
            unit_id,
            inventory_id,
        }),
    }
}

pub fn transfer_unit_inventory_to_corpse(
    world: &mut WorldData,
    unit_id: UnitId,
    corpse_id: CorpseId,
    inventory_id: InventoryId,
) -> Result<(), InventoryError> {
    transfer_inventory_owner(
        world.inventory_store_mut(),
        inventory_id,
        InventoryOwnerRef::Unit(unit_id),
        InventoryOwnerRef::Corpse(corpse_id),
    )
}

pub fn cleanup_unit_inventory_on_delete(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit: &UnitRecord,
) -> Result<RemovedInventoryContents, InventoryError> {
    let Some(inventory_id) = unit.inventory_id else {
        return Ok(RemovedInventoryContents {
            inventory_id: None,
            destroyed_instance_ids: Vec::new(),
        });
    };
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    remove_owned_inventory(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        InventoryOwnerRef::Unit(unit.id),
    )
}

#[cfg(test)]
mod i3_tests {
    use super::*;
    use crate::world::inventory::{InventoryOwnerRef, place_stack_first_fit};
    use crate::world::unit::death::step_unit_death_pipeline;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, CorpseSettings, Heightfield,
        InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, LocalPosition,
        UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition,
        create_unit_with_inventory, create_unit_with_ownership,
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions, starter_unit_definitions, step_corpse_lifecycle,
    };
    use bevy::prelude::Vec3;

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

    #[test]
    fn unit_without_profile_has_no_inventory() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut world = flat_world();
        let unit = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap();
        assert!(unit.inventory_id.is_none());
    }

    #[test]
    fn unit_with_profile_gets_empty_inventory() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut world = flat_world();
        let ctx = test_ctx();
        let unit = create_unit_with_inventory(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
            &ctx,
        )
        .unwrap();
        let inventory_id = unit.inventory_id.expect("inventory attached");
        let inventory = world.inventory_store().get(inventory_id).unwrap();
        assert!(inventory.placed_entries().is_empty());
        assert_eq!(*inventory.owner(), InventoryOwnerRef::Unit(unit.id));
        validate_unit_inventory_owner(&world, unit.id).unwrap();
    }

    #[test]
    fn death_transfers_inventory_to_corpse() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut world = flat_world();
        let ctx = test_ctx();
        let unit = create_unit_with_inventory(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(2.0, 2.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
            &ctx,
        )
        .unwrap();
        let inventory_id = unit.inventory_id.unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            10,
        )
        .unwrap();
        world.damage_unit(unit.id, 999).unwrap();
        let report = step_unit_death_pipeline(
            &mut world,
            &catalog,
            Some(&ctx),
            &CorpseSettings::default(),
            1,
        );
        assert!(world.get_unit(unit.id).is_none());
        assert_eq!(report.removed_unit_ids, vec![unit.id]);
        let corpse_id = report.corpse_ids[0];
        let corpse = world.corpse_store().get(corpse_id).unwrap();
        assert_eq!(corpse.inventory_id, Some(inventory_id));
        let inventory = world.inventory_store().get(inventory_id).unwrap();
        assert_eq!(*inventory.owner(), InventoryOwnerRef::Corpse(corpse_id));
        assert_eq!(inventory.total_mass_grams(), 10);
    }

    #[test]
    fn corpse_expiration_deletes_inventory_without_spill() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut world = flat_world();
        let ctx = test_ctx();
        let unit = create_unit_with_inventory(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(3.0, 3.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
            &ctx,
        )
        .unwrap();
        let inventory_id = unit.inventory_id.unwrap();
        world.damage_unit(unit.id, 999).unwrap();
        let report = step_unit_death_pipeline(
            &mut world,
            &catalog,
            Some(&ctx),
            &CorpseSettings::default(),
            1,
        );
        let corpse_id = report.corpse_ids[0];
        world
            .corpse_store_mut()
            .get_mut(corpse_id)
            .unwrap()
            .remaining_lifetime_ticks = 0;
        let lifecycle = step_corpse_lifecycle(&mut world, &ctx);
        assert!(lifecycle.expired_corpse_ids.contains(&corpse_id));
        assert!(world.corpse_store().get(corpse_id).is_none());
        assert!(world.inventory_store().get(inventory_id).is_none());
    }

    #[test]
    fn weight_queries_use_exact_inventory_cache() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut world = flat_world();
        let ctx = test_ctx();
        let unit = create_unit_with_inventory(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(4.0, 4.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
            &ctx,
        )
        .unwrap();
        assert_eq!(unit_inventory_weight_grams(&world, unit.id), 0);
        let inventory_id = unit.inventory_id.unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("iron_ore"),
            3,
        )
        .unwrap();
        assert_eq!(unit_inventory_weight_grams(&world, unit.id), 6_000);
        assert!(unit_over_reference_weight_grams(&world, &ctx, unit.id) == 0);
    }
}
