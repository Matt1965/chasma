//! Authoritative equipped-weapon resolution for combat (Slice 3).

use crate::world::inventory::InventoryEntryContents;
use crate::world::unit::{UnitCatalog, UnitOrderError, UnitRecord};
use crate::world::{ItemCatalog, WeaponCatalog, WeaponDefinition, WeaponDefinitionId, WorldData};

/// Resolve the weapon definition id a unit should use in combat.
pub fn effective_weapon_id_for_unit(
    world: &WorldData,
    unit: &UnitRecord,
    unit_catalog: &UnitCatalog,
    item_catalog: &ItemCatalog,
) -> Result<WeaponDefinitionId, UnitOrderError> {
    if let Some(weapon_id) = equipped_weapon_definition_id(world, unit, item_catalog) {
        return Ok(weapon_id);
    }
    let definition = unit_catalog
        .get(&unit.definition_id)
        .ok_or(UnitOrderError::DefinitionNotFound)?;
    Ok(definition.default_weapon_id.clone())
}

/// Resolve the weapon definition a unit should use in combat.
pub fn effective_weapon_for_unit<'a>(
    world: &WorldData,
    unit: &UnitRecord,
    unit_catalog: &'a UnitCatalog,
    item_catalog: &'a ItemCatalog,
    weapon_catalog: &'a WeaponCatalog,
) -> Result<&'a WeaponDefinition, UnitOrderError> {
    let weapon_id = effective_weapon_id_for_unit(world, unit, unit_catalog, item_catalog)?;
    let weapon = weapon_catalog
        .get(&weapon_id)
        .ok_or(UnitOrderError::MissingWeapon)?;
    if !weapon.enabled {
        return Err(UnitOrderError::MissingWeapon);
    }
    Ok(weapon)
}

fn equipped_weapon_definition_id(
    world: &WorldData,
    unit: &UnitRecord,
    item_catalog: &ItemCatalog,
) -> Option<WeaponDefinitionId> {
    let equipment = unit.equipment?;
    let weapon_slot = equipment.weapon;
    let record = world.inventory_store().get(weapon_slot)?;
    let entry = record.placed_entries().first()?;
    let instance_id = match &entry.contents {
        InventoryEntryContents::Unique { item_instance_id } => *item_instance_id,
        _ => return None,
    };
    let instance = world.item_instance_store().get(instance_id)?;
    let item = item_catalog.get(&instance.definition_id)?;
    item.weapon_definition_id.clone()
}
