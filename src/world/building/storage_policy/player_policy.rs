//! Player-facing storage policy mutations (owned building menu).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::item::ItemCategoryCatalog;
use crate::world::{BuildingId, ItemCategoryId, WorldData};

use super::api::{building_is_storage_capable, mark_settlement_storage_logistics_wakeup};
use super::policy::BuildingStoragePolicy;
use crate::world::building::operation::ProductionCommandError;

pub fn apply_player_storage_category_accepted(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    category_id: ItemCategoryId,
    accepted: bool,
) -> Result<(), ProductionCommandError> {
    require_storage_capable_building(world, building_catalog, building_id)?;
    let policy = world
        .building_storage_policy_store_mut()
        .policy_mut(building_id);
    policy.set_category_accepted(&category_id, accepted);
    mark_settlement_storage_logistics_wakeup(world, building_catalog, building_id);
    Ok(())
}

pub fn apply_player_storage_accept_all(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
) -> Result<(), ProductionCommandError> {
    require_storage_capable_building(world, building_catalog, building_id)?;
    world
        .building_storage_policy_store_mut()
        .policy_mut(building_id)
        .accept_all_categories();
    mark_settlement_storage_logistics_wakeup(world, building_catalog, building_id);
    Ok(())
}

pub fn apply_player_storage_clear_all(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    category_catalog: &ItemCategoryCatalog,
    building_id: BuildingId,
) -> Result<(), ProductionCommandError> {
    require_storage_capable_building(world, building_catalog, building_id)?;
    let category_ids = category_catalog
        .enabled_definitions()
        .map(|category| category.id.clone())
        .collect::<Vec<_>>();
    world
        .building_storage_policy_store_mut()
        .policy_mut(building_id)
        .deny_all_categories(category_ids);
    mark_settlement_storage_logistics_wakeup(world, building_catalog, building_id);
    Ok(())
}

fn require_storage_capable_building(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
) -> Result<(), ProductionCommandError> {
    let record = world
        .get_building(building_id)
        .ok_or(ProductionCommandError::BuildingNotFound(building_id))?;
    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or(ProductionCommandError::BuildingNotFound(building_id))?;
    if !building_is_storage_capable(definition) {
        return Err(ProductionCommandError::BuildingNotFound(building_id));
    }
    Ok(())
}

pub fn effective_storage_category_accepted(
    policy: &BuildingStoragePolicy,
    category_id: &ItemCategoryId,
) -> bool {
    policy.accepts_category(category_id)
}
