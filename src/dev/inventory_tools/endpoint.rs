//! Generic inventory container resolution for dev tools (DV0).

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::dev_mode::{DevInventoryEndpoint, DevInventoryToolState};
use crate::ui::gameplay::primary_selected_unit;
use crate::units::input::SelectedUnits;
use crate::world::{BuildingId, InventoryId, ItemPileId, UnitId, WorldData};

/// Unit/building/pile the dev inventory tools should target.
pub fn resolve_target_unit(
    world_selection: &WorldSelectionState,
    selection: &SelectedUnits,
) -> Option<UnitId> {
    if world_selection.category == WorldSelectionCategory::Units {
        world_selection.primary_unit(selection)
    } else {
        primary_selected_unit(selection)
    }
}

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

/// Pick the active dev inventory endpoint from shared selection.
pub fn resolve_active_endpoint(
    world: &WorldData,
    world_selection: &WorldSelectionState,
    selection: &SelectedUnits,
    dev_state: &DevInventoryToolState,
) -> Option<DevInventoryEndpoint> {
    let endpoints = resolve_inspector_endpoints(world, world_selection, selection);
    if endpoints.is_empty() {
        return None;
    }
    let index = dev_state.selected_endpoint_index % endpoints.len();
    Some(endpoints[index].endpoint)
}

pub fn resolve_inspector_endpoints(
    world: &WorldData,
    world_selection: &WorldSelectionState,
    selection: &SelectedUnits,
) -> Vec<DevInventoryEndpointInfo> {
    let mut out = Vec::new();

    if let Some(unit_id) = resolve_target_unit(world_selection, selection) {
        if let Some(inventory_id) = world.get_unit(unit_id).and_then(|u| u.inventory_id) {
            out.push(DevInventoryEndpointInfo {
                endpoint: DevInventoryEndpoint::Grid(inventory_id),
                label: format!("Unit #{} inventory", unit_id.raw()),
                owner_kind: "unit",
            });
        }
    }

    if let Some(building_id) = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten()
    {
        if let Some(inventory_id) = world.get_building(building_id).and_then(|b| b.inventory_id) {
            out.push(DevInventoryEndpointInfo {
                endpoint: DevInventoryEndpoint::Grid(inventory_id),
                label: format!("Building #{} inventory", building_id.raw()),
                owner_kind: "building",
            });
        }
    }

    if let Some(pile_id) = (world_selection.category == WorldSelectionCategory::ItemPile)
        .then_some(world_selection.pile_id)
        .flatten()
    {
        out.push(DevInventoryEndpointInfo {
            endpoint: DevInventoryEndpoint::Pile(pile_id),
            label: format!("Ground pile {pile_id:?}"),
            owner_kind: "pile",
        });
    }

    out
}

pub fn building_inventory_endpoint(
    world: &WorldData,
    building_id: BuildingId,
) -> Option<DevInventoryEndpoint> {
    world
        .get_building(building_id)
        .and_then(|b| b.inventory_id)
        .map(DevInventoryEndpoint::Grid)
}

pub fn unit_inventory_endpoint(world: &WorldData, unit_id: UnitId) -> Option<DevInventoryEndpoint> {
    world
        .get_unit(unit_id)
        .and_then(|u| u.inventory_id)
        .map(DevInventoryEndpoint::Grid)
}

pub fn pile_endpoint(_world: &WorldData, pile_id: ItemPileId) -> DevInventoryEndpoint {
    DevInventoryEndpoint::Pile(pile_id)
}

/// Nearest pile at a world position (dev inspector pick).
pub fn nearest_pile_at_position(
    world: &WorldData,
    position: crate::world::WorldPosition,
    settings: &crate::world::ItemPileSettings,
) -> Option<ItemPileId> {
    let chunk = crate::world::ChunkId::new(position.chunk);
    let piles: Vec<_> = world.item_pile_store().piles_in_chunk(chunk).to_vec();
    crate::world::item_piles_near(&piles, position, crate::world::SpaceId::SURFACE, settings)
        .into_iter()
        .next()
        .map(|pile| pile.id)
}
