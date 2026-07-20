//! Binding resolution and role query helpers (EP4).

use crate::world::BuildingId;
use crate::world::InventoryId;
use crate::world::WorldData;

use super::binding::BuildingInventoryBinding;
use super::binding_id::BuildingInventoryBindingId;
use super::role::BuildingInventoryRole;
use super::store::BuildingInventoryBindingStore;

/// Resolve a building binding to its authoritative [`InventoryId`] (EP4).
pub fn resolve_building_inventory_binding(
    store: &BuildingInventoryBindingStore,
    building_id: BuildingId,
    binding_id: &BuildingInventoryBindingId,
) -> Option<InventoryId> {
    store.resolve_inventory(building_id, binding_id)
}

/// List all bindings on a building (EP4).
pub fn building_inventory_bindings(
    store: &BuildingInventoryBindingStore,
    building_id: BuildingId,
) -> &[BuildingInventoryBinding] {
    store
        .get(building_id)
        .map(|set| set.bindings())
        .unwrap_or(&[])
}

/// Find all bindings with a broad role — returns all matches, never picks one (EP4).
pub fn building_inventories_with_role<'a>(
    store: &'a BuildingInventoryBindingStore,
    building_id: BuildingId,
    role: BuildingInventoryRole,
) -> Vec<&'a BuildingInventoryBinding> {
    store
        .get(building_id)
        .map(|set| set.bindings_with_role(role).collect())
        .unwrap_or_default()
}

/// Explicit default binding when authored (EP4).
pub fn default_building_inventory_binding<'a>(
    store: &'a BuildingInventoryBindingStore,
    building_id: BuildingId,
) -> Option<&'a BuildingInventoryBinding> {
    store.get(building_id).and_then(|set| set.default_binding())
}

/// Compatibility accessor for legacy single-inventory code paths (EP4).
pub fn primary_building_inventory_id(
    world: &WorldData,
    building_id: BuildingId,
) -> Option<InventoryId> {
    world
        .building_inventory_binding_store()
        .get(building_id)
        .and_then(|set| set.default_inventory_id())
        .or_else(|| world.get_building(building_id).and_then(|record| record.inventory_id))
}
