//! Generic inventory container resolution for dev tools (DV0).

use crate::dev::dev_mode::DevInventoryEndpoint;
use crate::dev::inspector::WorldInspectorState;
use crate::world::{BuildingId, InventoryId, ItemPileId, UnitId, WorldData};

/// Resolved container with human-readable context for the dev panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevInventoryEndpointInfo {
    pub endpoint: DevInventoryEndpoint,
    pub label: String,
    pub owner_kind: &'static str,
}

impl DevInventoryEndpoint {
    pub fn label_suffix(self, world: &WorldData) -> String {
        match self {
            Self::Grid(id) => {
                let entries = world
                    .inventory_store()
                    .get(id)
                    .map(|record| record.placed_entries().len())
                    .unwrap_or(0);
                format!("inv#{:?} ({entries} entries)", id)
            }
            Self::Pile(id) => format!("pile#{:?}", id),
        }
    }
}

/// List inventory containers reachable from the current inspector selection.
pub fn resolve_inspector_endpoints(
    world: &WorldData,
    inspector: &WorldInspectorState,
) -> Vec<DevInventoryEndpointInfo> {
    let mut endpoints = Vec::new();
    let mut seen_inventories = Vec::new();

    if let Some(unit_id) = inspector.selected_unit {
        push_unit_inventory(world, unit_id, &mut endpoints, &mut seen_inventories);
    }

    if let Some(building_id) = inspector.selected_building {
        push_building_inventories(world, building_id, &mut endpoints, &mut seen_inventories);
    }

    if let Some(pile_id) = inspector.selected_pile {
        push_pile(world, pile_id, &mut endpoints);
    }

    endpoints
}

fn push_unique_grid(
    inventory_id: InventoryId,
    label: String,
    owner_kind: &'static str,
    endpoints: &mut Vec<DevInventoryEndpointInfo>,
    seen: &mut Vec<InventoryId>,
) {
    if seen.contains(&inventory_id) {
        return;
    }
    seen.push(inventory_id);
    endpoints.push(DevInventoryEndpointInfo {
        endpoint: DevInventoryEndpoint::Grid(inventory_id),
        label,
        owner_kind,
    });
}

fn push_unit_inventory(
    world: &WorldData,
    unit_id: UnitId,
    endpoints: &mut Vec<DevInventoryEndpointInfo>,
    seen: &mut Vec<InventoryId>,
) {
    let Some(unit) = world.get_unit(unit_id) else {
        return;
    };
    let Some(inventory_id) = unit.inventory_id else {
        return;
    };
    push_unique_grid(
        inventory_id,
        format!("Unit #{} inventory", unit_id.raw()),
        "unit",
        endpoints,
        seen,
    );
}

fn push_building_inventories(
    world: &WorldData,
    building_id: BuildingId,
    endpoints: &mut Vec<DevInventoryEndpointInfo>,
    seen: &mut Vec<InventoryId>,
) {
    let Some(building) = world.get_building(building_id) else {
        return;
    };

    if let Some(inventory_id) = building.inventory_id {
        push_unique_grid(
            inventory_id,
            format!("Building #{} primary", building_id.raw()),
            "building",
            endpoints,
            seen,
        );
    }

    if let Some(binding_set) = world.building_inventory_binding_store().get(building_id) {
        for binding in binding_set.bindings() {
            push_unique_grid(
                binding.inventory_id,
                format!(
                    "Building #{} `{}` ({:?})",
                    building_id.raw(),
                    binding.binding_id.as_str(),
                    binding.role
                ),
                "building",
                endpoints,
                seen,
            );
        }
    }
}

fn push_pile(world: &WorldData, pile_id: ItemPileId, endpoints: &mut Vec<DevInventoryEndpointInfo>) {
    if world.item_pile_store().get(pile_id).is_none() {
        return;
    }
    endpoints.push(DevInventoryEndpointInfo {
        endpoint: DevInventoryEndpoint::Pile(pile_id),
        label: format!("Ground pile #{}", pile_id.raw()),
        owner_kind: "pile",
    });
}

/// Nearest pile at a world position (dev inspector pick).
pub fn nearest_pile_at_position(
    world: &WorldData,
    position: crate::world::WorldPosition,
    settings: &crate::world::ItemPileSettings,
) -> Option<ItemPileId> {
    let chunk = crate::world::ChunkId::new(position.chunk);
    let piles: Vec<_> = world
        .item_pile_store()
        .piles_in_chunk(chunk)
        .to_vec();
    crate::world::item_piles_near(&piles, position, crate::world::SpaceId::SURFACE, settings)
        .into_iter()
        .next()
        .map(|pile| pile.id)
}
