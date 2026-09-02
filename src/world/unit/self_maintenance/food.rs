//! Food discovery and consumption for individual self-maintenance (ADR-134).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::{InventoryCatalogCtx, InventoryEntryContents, consume_stack_item};
use crate::world::item::{ItemCatalog, ItemCategoryId, ItemDefinitionId};
use crate::world::settlement::SettlementId;
use crate::world::settlement::building_advertises_settlement_supply;
use crate::world::{
    BuildingId, BuildingInteractionProfileCatalog, INTERACTION_WORK_RANGE_METERS, InventoryId,
    UnitId, WorldData, WorldPosition, interaction_point_world_position,
};

use super::nutrition::{NutritionProfile, UnitNutritionState, restore_nutrition};
use super::state::FoodSourceRef;

pub const FOOD_CATEGORY_ID: &str = "food";

/// Whether an item definition is edible food for hunger purposes.
pub fn is_edible_food(item_catalog: &ItemCatalog, item_id: &ItemDefinitionId) -> bool {
    let Some(def) = item_catalog.get(item_id) else {
        return false;
    };
    def.category_id == ItemCategoryId::new(FOOD_CATEGORY_ID) && def.nutrition > 0
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdibleStack {
    pub inventory_id: InventoryId,
    pub building_id: Option<BuildingId>,
    pub item_definition_id: ItemDefinitionId,
    pub entry_index: usize,
    pub nutrition: u32,
    pub interaction_position: WorldPosition,
    pub distance_sq: f32,
}

/// Find the nearest edible stack in a unit's own inventory.
pub fn find_edible_in_inventory(
    world: &WorldData,
    item_catalog: &ItemCatalog,
    inventory_id: InventoryId,
    unit_position: WorldPosition,
    layout: crate::world::ChunkLayout,
) -> Option<EdibleStack> {
    let inventory = world.inventory_store().get(inventory_id)?;
    let unit_global = unit_position.to_global(layout);
    let mut best: Option<EdibleStack> = None;
    for (entry_index, entry) in inventory.placed_entries().iter().enumerate() {
        let InventoryEntryContents::Stack {
            item_definition_id,
            quantity,
        } = &entry.contents
        else {
            continue;
        };
        if *quantity == 0 || !is_edible_food(item_catalog, item_definition_id) {
            continue;
        }
        let nutrition = item_catalog.get(item_definition_id)?.nutrition;
        let candidate = EdibleStack {
            inventory_id,
            building_id: None,
            item_definition_id: item_definition_id.clone(),
            entry_index: entry_index,
            nutrition,
            interaction_position: unit_position,
            distance_sq: 0.0,
        };
        update_nearest_edible(&mut best, candidate, &unit_global, layout);
    }
    best
}

/// Find the nearest edible stack in accessible settlement storage inventories.
pub fn find_nearest_settlement_edible(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    item_catalog: &ItemCatalog,
    settlement_id: SettlementId,
    unit_position: WorldPosition,
    layout: crate::world::ChunkLayout,
) -> Option<EdibleStack> {
    let unit_global = unit_position.to_global(layout);
    let mut best: Option<EdibleStack> = None;
    let building_ids = world
        .settlement_store()
        .buildings_for_settlement(settlement_id);
    for building_id in building_ids {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if record.settlement_id != Some(settlement_id) {
            continue;
        }
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if !building_advertises_settlement_supply(definition) {
            continue;
        }
        let Some(bindings) = world.building_inventory_binding_store().get(building_id) else {
            continue;
        };
        let interaction_pos =
            building_food_interaction_position(world, building_id, interaction_catalog, layout);
        for binding in bindings.bindings() {
            if !binding.role.advertises_logistics_supply() {
                continue;
            }
            let inventory_id = binding.inventory_id;
            let Some(inventory) = world.inventory_store().get(inventory_id) else {
                continue;
            };
            for (entry_index, entry) in inventory.placed_entries().iter().enumerate() {
                let InventoryEntryContents::Stack {
                    item_definition_id,
                    quantity,
                } = &entry.contents
                else {
                    continue;
                };
                if *quantity == 0 || !is_edible_food(item_catalog, item_definition_id) {
                    continue;
                }
                let nutrition = item_catalog.get(item_definition_id)?.nutrition;
                let candidate = EdibleStack {
                    inventory_id,
                    building_id: Some(building_id),
                    item_definition_id: item_definition_id.clone(),
                    entry_index: entry_index,
                    nutrition,
                    interaction_position: interaction_pos,
                    distance_sq: 0.0,
                };
                update_nearest_edible(&mut best, candidate, &unit_global, layout);
            }
            let _ = binding.binding_id.clone();
        }
    }
    best
}

pub(crate) fn building_food_interaction_position(
    world: &WorldData,
    building_id: BuildingId,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    layout: crate::world::ChunkLayout,
) -> WorldPosition {
    let Some(building) = world.get_building(building_id) else {
        return WorldPosition::new(
            crate::world::ChunkCoord::new(0, 0),
            crate::world::LocalPosition::new(bevy::prelude::Vec3::ZERO),
        );
    };
    if let Some(profile) = interaction_catalog.get(building.definition_id.as_str()) {
        if let Some(point) = profile.points.first() {
            return interaction_point_world_position(building, layout, point);
        }
    }
    building.placement.position
}

fn update_nearest_edible(
    best: &mut Option<EdibleStack>,
    mut candidate: EdibleStack,
    unit_global: &bevy::prelude::Vec3,
    layout: crate::world::ChunkLayout,
) {
    let target = candidate.interaction_position.to_global(layout);
    let dx = target.x - unit_global.x;
    let dz = target.z - unit_global.z;
    candidate.distance_sq = dx * dx + dz * dz;
    let replace = match best {
        None => true,
        Some(current) => {
            candidate.distance_sq < current.distance_sq
                || (candidate.distance_sq == current.distance_sq
                    && candidate.item_definition_id.as_str() < current.item_definition_id.as_str())
        }
    };
    if replace {
        *best = Some(candidate);
    }
}

pub fn unit_near_food_source(
    unit_position: WorldPosition,
    source_position: WorldPosition,
    layout: crate::world::ChunkLayout,
) -> bool {
    let unit = unit_position.to_global(layout);
    let target = source_position.to_global(layout);
    let dx = unit.x - target.x;
    let dz = unit.z - target.z;
    (dx * dx + dz * dz).sqrt() <= INTERACTION_WORK_RANGE_METERS
}

/// Consume one edible item stack quantity from an inventory and restore unit nutrition.
pub fn eat_one_from_inventory(
    world: &mut WorldData,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
    nutrition_state: &mut UnitNutritionState,
    profile: &NutritionProfile,
    inventory_id: InventoryId,
    item_id: &ItemDefinitionId,
    item_catalog: &ItemCatalog,
) -> bool {
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let consumed = consume_stack_item(
        inventory_store,
        instance_store,
        inventory_ctx,
        inventory_id,
        item_id,
        1,
    );
    let Ok(consumed) = consumed else {
        return false;
    };
    if consumed == 0 {
        return false;
    }
    let nutrition = item_catalog
        .get(item_id)
        .map(|def| def.nutrition as f32)
        .unwrap_or(0.0);
    restore_nutrition(nutrition_state, nutrition, profile);
    let _ = unit_id;
    true
}

pub fn edible_to_source(edible: &EdibleStack) -> FoodSourceRef {
    match edible.building_id {
        Some(building_id) => FoodSourceRef::SettlementStorage {
            inventory_id: edible.inventory_id,
            building_id,
        },
        None => FoodSourceRef::OwnInventory {
            inventory_id: edible.inventory_id,
        },
    }
}

/// Pick the best food source for a settlement member: own inventory first, then settlement storage.
pub fn select_food_source(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    item_catalog: &ItemCatalog,
    unit_id: UnitId,
    settlement_id: Option<SettlementId>,
) -> Option<EdibleStack> {
    let unit = world.get_unit(unit_id)?;
    let layout = world.layout();
    let position = unit.placement.position;
    if let Some(inventory_id) = unit.inventory_id {
        if let Some(edible) =
            find_edible_in_inventory(world, item_catalog, inventory_id, position, layout)
        {
            return Some(edible);
        }
    }
    let settlement_id = settlement_id?;
    find_nearest_settlement_edible(
        world,
        building_catalog,
        interaction_catalog,
        item_catalog,
        settlement_id,
        position,
        layout,
    )
}
