//! Unified world inventory validation (ADR-094 I8).

use super::{
    InventoryCatalogCtx, InventoryError, InventoryId, InventoryInvariantReport, InventoryOwnerRef,
    resolve_instance_definition, validate_inventory_stores,
};
use crate::world::{
    ItemPileInvariantReport, WorldData, validate_item_instance_locations, validate_item_pile_store,
};

/// Aggregated validation report for all inventory-related world state (ADR-094 I8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorldInventoryValidationReport {
    pub inventory: InventoryInvariantReport,
    pub piles: ItemPileInvariantReport,
    pub location_errors: Vec<String>,
    pub link_errors: Vec<String>,
    pub treasury_errors: Vec<String>,
}

impl WorldInventoryValidationReport {
    pub fn is_ok(&self) -> bool {
        self.inventory.is_ok()
            && self.piles.is_ok()
            && self.location_errors.is_empty()
            && self.link_errors.is_empty()
            && self.treasury_errors.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.inventory.errors.len()
            + self.piles.errors.len()
            + self.location_errors.len()
            + self.link_errors.len()
            + self.treasury_errors.len()
    }
}

/// Authoritative validation entry point for I1–I7 inventory world state (ADR-094 I8).
pub fn validate_world_inventory_state(
    world: &WorldData,
    ctx: &InventoryCatalogCtx<'_>,
) -> WorldInventoryValidationReport {
    let mut report = WorldInventoryValidationReport::default();
    report.inventory =
        validate_inventory_stores(world.inventory_store(), world.item_instance_store(), ctx);
    report.piles = validate_item_pile_store(world.item_pile_store());
    let location_report =
        validate_item_instance_locations(world.item_instance_store(), world.item_pile_store());
    report.location_errors.extend(location_report.errors);
    report.link_errors.extend(validate_owner_links(world));
    report
        .treasury_errors
        .extend(validate_settlement_links(world));
    report
}

fn validate_owner_links(world: &WorldData) -> Vec<String> {
    let mut errors = Vec::new();
    for inventory_id in world.inventory_store().sorted_inventory_ids() {
        let Some(record) = world.inventory_store().get(inventory_id) else {
            errors.push(format!("missing inventory record {inventory_id:?}"));
            continue;
        };
        match record.owner() {
            InventoryOwnerRef::Detached => {}
            InventoryOwnerRef::Unit(unit_id) => {
                if let Some(unit) = world.get_unit(*unit_id) {
                    if unit.inventory_id != Some(inventory_id) {
                        errors.push(format!(
                            "unit {unit_id:?} inventory_id does not reference inventory {inventory_id:?}"
                        ));
                    }
                } else {
                    errors.push(format!(
                        "inventory {inventory_id:?} owned by missing unit {unit_id:?}"
                    ));
                }
            }
            InventoryOwnerRef::Building(building_id) => {
                if let Some(building) = world.get_building(*building_id) {
                    if building.inventory_id != Some(inventory_id) {
                        errors.push(format!(
                            "building {building_id:?} inventory_id does not reference inventory {inventory_id:?}"
                        ));
                    }
                } else {
                    errors.push(format!(
                        "inventory {inventory_id:?} owned by missing building {building_id:?}"
                    ));
                }
            }
            InventoryOwnerRef::Corpse(corpse_id) => {
                if let Some(corpse) = world.corpse_store().get(*corpse_id) {
                    if corpse.inventory_id != Some(inventory_id) {
                        errors.push(format!(
                            "corpse {corpse_id:?} inventory_id does not reference inventory {inventory_id:?}"
                        ));
                    }
                } else {
                    errors.push(format!(
                        "inventory {inventory_id:?} owned by missing corpse {corpse_id:?}"
                    ));
                }
            }
        }
    }

    for unit_id in world.sorted_unit_ids() {
        if let Some(unit) = world.get_unit(unit_id) {
            if let Some(inventory_id) = unit.inventory_id {
                if let Some(record) = world.inventory_store().get(inventory_id) {
                    if !matches!(record.owner(), InventoryOwnerRef::Unit(unit_id)) {
                        errors.push(format!(
                            "unit {unit_id:?} references inventory {inventory_id:?} with mismatched owner"
                        ));
                    }
                } else {
                    errors.push(format!(
                        "unit {unit_id:?} references missing inventory {inventory_id:?}"
                    ));
                }
            }
        }
    }

    for building_id in world.sorted_building_ids() {
        if let Some(building) = world.get_building(building_id) {
            if let Some(inventory_id) = building.inventory_id {
                if let Some(record) = world.inventory_store().get(inventory_id) {
                    if !matches!(record.owner(), InventoryOwnerRef::Building(building_id)) {
                        errors.push(format!(
                            "building {building_id:?} references inventory {inventory_id:?} with mismatched owner"
                        ));
                    }
                } else {
                    errors.push(format!(
                        "building {building_id:?} references missing inventory {inventory_id:?}"
                    ));
                }
            }
        }
    }

    for corpse_id in world.corpse_store().sorted_corpse_ids() {
        if let Some(corpse) = world.corpse_store().get(corpse_id) {
            if let Some(inventory_id) = corpse.inventory_id {
                if let Some(record) = world.inventory_store().get(inventory_id) {
                    if !matches!(record.owner(), InventoryOwnerRef::Corpse(corpse_id)) {
                        errors.push(format!(
                            "corpse {corpse_id:?} references inventory {inventory_id:?} with mismatched owner"
                        ));
                    }
                } else {
                    errors.push(format!(
                        "corpse {corpse_id:?} references missing inventory {inventory_id:?}"
                    ));
                }
            }
        }
    }

    errors
}

fn validate_settlement_links(world: &WorldData) -> Vec<String> {
    let mut errors = Vec::new();
    for settlement_id in world.settlement_store().sorted_settlement_ids() {
        let Some(settlement) = world.settlement_store().get_settlement(settlement_id) else {
            errors.push(format!("missing settlement {settlement_id:?}"));
            continue;
        };
        if world.get_building(settlement.anchor_building_id).is_none() {
            errors.push(format!(
                "settlement {settlement_id:?} anchor building {:?} missing",
                settlement.anchor_building_id
            ));
        }
        let Some(treasury_id) = world
            .settlement_store()
            .treasury_for_settlement(settlement_id)
        else {
            errors.push(format!(
                "settlement {settlement_id:?} has no treasury index"
            ));
            continue;
        };
        if settlement.treasury_id != treasury_id {
            errors.push(format!(
                "settlement {settlement_id:?} treasury_id mismatch (record vs index)"
            ));
        }
        if world.settlement_store().get_treasury(treasury_id).is_none() {
            errors.push(format!(
                "settlement {settlement_id:?} references missing treasury {treasury_id:?}"
            ));
        }
    }
    errors
}

/// Rebuild occupancy and mass caches for every inventory (ADR-094 I8).
pub fn rebuild_all_inventory_derived(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
) -> Result<(), InventoryError> {
    let inventory_ids = world.inventory_store().sorted_inventory_ids();
    for inventory_id in inventory_ids {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let Some(record) = inventory_store.get_mut(inventory_id) else {
            return Err(InventoryError::InventoryNotFound(inventory_id));
        };
        record.rebuild_derived(ctx, |id| resolve_instance_definition(instance_store, id))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingCatalog, BuildingInteractionProfileCatalog, BuildingOwnership,
        BuildingSource, ChunkCoord, ChunkData, ChunkLayout, Heightfield, InventoryProfileCatalog,
        ItemCatalog, ItemCategoryCatalog, LocalPosition, SettlementOwnership, UnitCatalog,
        UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition, create_building,
        create_settlement_with_treasury, create_unit_with_inventory, physical_gold_item_id,
        place_stack_first_fit, starter_building_definitions, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
    };
    use bevy::prelude::{Quat, Vec3};

    fn test_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn test_ctx<'a>(
        items: &'a ItemCatalog,
        categories: &'a ItemCategoryCatalog,
        profiles: &'a InventoryProfileCatalog,
    ) -> InventoryCatalogCtx<'a> {
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    #[test]
    fn validate_world_inventory_state_passes_fixture() {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let building_categories = crate::world::BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &building_categories)
                .unwrap();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut world = test_world();
        let ctx = test_ctx(&items, &categories, &profiles);
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            &ctx,
        )
        .unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            unit.inventory_id.unwrap(),
            physical_gold_item_id(),
            5,
        )
        .unwrap();
        let building = create_building(
            &building_catalog,
            &mut world,
            &crate::world::BuildingDefinitionId::new("settlement_core"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(2.0, 0.0, 2.0)),
            ),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
        )
        .unwrap();
        create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            building.id,
            "Town",
            SettlementOwnership::player_default(),
            building.placement.position,
            0,
        )
        .unwrap();
        let report = validate_world_inventory_state(&world, &ctx);
        assert!(report.is_ok(), "{report:?}");
    }
}
