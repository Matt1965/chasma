//! Dev Mode item pile tools (ADR-090 I4) — UI-driven (Slice 12).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::units::input::SelectedUnits;
use crate::world::{
    Affiliation, ChunkId, InventoryCatalogCtx, InventoryEntryContents, InventoryProfileCatalog,
    ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, ItemInstanceMetadata, ItemPileSettings,
    ItemPileSource, PileOwnership, SpaceId, TransferPlacementPolicy, WorldData,
    WorldItemPileRecord, create_item_instance, drop_stack_from_inventory,
    drop_unit_inventory_entry, half_stack_quantity, loot_corpse_entry, pickup_pile_into_inventory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PileHarnessAction {
    ValidateWorld,
    SpawnGoldPile,
    DropEntry,
    DropOne,
    DropHalf,
    PickupPile,
    LootCorpse,
}

impl PileHarnessAction {
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::ValidateWorld => "Validate all world inventories. Dev diagnostic.",
            Self::SpawnGoldPile => {
                "Spawn a gold pile (×5) at the selected unit's feet. Requires a selected unit."
            }
            Self::DropEntry => "Drop inventory entry 0 from the selected unit to the ground.",
            Self::DropOne => "Drop one item from entry 0.",
            Self::DropHalf => "Drop half of entry 0 stack quantity.",
            Self::PickupPile => "Pick up the first world pile into the selected unit inventory.",
            Self::LootCorpse => "Loot first entry from the first corpse with inventory.",
        }
    }
}

pub fn format_pile_harness_detail(
    world: &WorldData,
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
    message: &str,
) -> String {
    let unit_line = world_selection
        .primary_unit(selected_units)
        .map(|id| format!("Selected unit: {id:?}"))
        .unwrap_or_else(|| "Selected unit: none (Alt+click unit)".into());
    let pile_count = world.item_pile_store().sorted_item_pile_ids().len();
    format!("{unit_line}\nPiles in world: {pile_count}\n{message}")
}

pub fn apply_pile_harness_action(
    action: PileHarnessAction,
    world: &mut WorldData,
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
    ctx: &InventoryCatalogCtx,
    settings: &ItemPileSettings,
    tick: u64,
) -> String {
    match action {
        PileHarnessAction::ValidateWorld => {
            let report = crate::world::validate_world_inventory_state(world, ctx);
            if report.is_ok() {
                format!(
                    "World inventory validation OK ({} inventories)",
                    world.inventory_store().len()
                )
            } else {
                format!("World inventory validation: {report:?}")
            }
        }
        _ => {
            let Some(unit_id) = world_selection.primary_unit(selected_units) else {
                return "Select a unit (Alt+click)".to_string();
            };
            let Some(unit) = world.get_unit(unit_id).cloned() else {
                return "Selected unit missing".to_string();
            };
            match action {
                PileHarnessAction::SpawnGoldPile => {
                    match spawn_stack_pile_at(
                        world,
                        ItemDefinitionId::new("gold"),
                        5,
                        unit.placement.position,
                        tick,
                    ) {
                        Ok(pile_id) => format!("Spawned pile `{pile_id}` with gold x5"),
                        Err(err) => err,
                    }
                }
                PileHarnessAction::DropEntry => {
                    match drop_unit_inventory_entry(world, ctx, settings, unit_id, 0, None, tick) {
                        Ok(report) => format!(
                            "Dropped {} (merged {}, new piles {:?})",
                            report.removed_from_inventory,
                            report.merged_into_existing_piles,
                            report.created_pile_ids
                        ),
                        Err(err) => err.to_string(),
                    }
                }
                PileHarnessAction::DropOne => {
                    match drop_unit_inventory_entry(world, ctx, settings, unit_id, 0, Some(1), tick)
                    {
                        Ok(report) => format!("Dropped one: {report:?}"),
                        Err(err) => err.to_string(),
                    }
                }
                PileHarnessAction::DropHalf => {
                    let inventory_id = match unit.inventory_id {
                        Some(id) => id,
                        None => return "Unit has no inventory".to_string(),
                    };
                    let entry = world
                        .inventory_store()
                        .get(inventory_id)
                        .and_then(|record| record.placed_entries().first())
                        .map(|entry| entry.contents.clone());
                    let Some(InventoryEntryContents::Stack { quantity, .. }) = entry else {
                        return "Entry 0 is not a stack".to_string();
                    };
                    let drop_qty = half_stack_quantity(quantity);
                    match drop_stack_from_inventory(
                        world,
                        ctx,
                        settings,
                        inventory_id,
                        0,
                        drop_qty,
                        unit.placement.position,
                        unit.current_space_id,
                        PileOwnership::from_unit(&unit),
                        tick,
                    ) {
                        Ok(report) => format!("Dropped half ({drop_qty}): {report:?}"),
                        Err(err) => err.to_string(),
                    }
                }
                PileHarnessAction::PickupPile => {
                    let pile_id = world
                        .item_pile_store()
                        .sorted_item_pile_ids()
                        .first()
                        .copied();
                    let Some(pile_id) = pile_id else {
                        return "No piles to pick up".to_string();
                    };
                    let Some(inventory_id) = unit.inventory_id else {
                        return "Unit has no inventory".to_string();
                    };
                    match pickup_pile_into_inventory(
                        world,
                        ctx,
                        pile_id,
                        inventory_id,
                        None,
                        unit.owner_id,
                        unit.team_id,
                        unit.affiliation,
                    ) {
                        Ok(report) => format!("Pickup: {report:?}"),
                        Err(err) => err.to_string(),
                    }
                }
                PileHarnessAction::LootCorpse => {
                    let corpse = world
                        .corpse_store()
                        .sorted_corpse_ids()
                        .into_iter()
                        .filter_map(|id| world.corpse_store().get(id).cloned())
                        .find(|corpse| corpse.inventory_id.is_some());
                    let Some(corpse) = corpse else {
                        return "No lootable corpse".to_string();
                    };
                    let corpse_inventory = corpse.inventory_id.unwrap();
                    let Some(unit_inventory) = unit.inventory_id else {
                        return "Unit has no inventory".to_string();
                    };
                    let (inventory_store, instance_store) = world.inventory_runtime_mut();
                    match loot_corpse_entry(
                        inventory_store,
                        instance_store,
                        ctx,
                        corpse_inventory,
                        0,
                        unit_inventory,
                        None,
                        TransferPlacementPolicy::MergeThenFirstFit,
                    ) {
                        Ok(report) => format!("Looted: {report:?}"),
                        Err(err) => err.to_string(),
                    }
                }
                PileHarnessAction::ValidateWorld => unreachable!(),
            }
        }
    }
}

fn spawn_stack_pile_at(
    world: &mut WorldData,
    item_definition_id: ItemDefinitionId,
    quantity: u32,
    position: crate::world::WorldPosition,
    tick: u64,
) -> Result<crate::world::ItemPileId, String> {
    let chunk = ChunkId::new(position.chunk);
    let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
    let record = WorldItemPileRecord::new_stack(
        pile_id,
        position,
        SpaceId::SURFACE,
        item_definition_id,
        quantity,
        None,
        None,
        Affiliation::Player,
        ItemPileSource::DevSpawned,
        tick,
    );
    world
        .item_pile_store_mut()
        .insert(chunk, record)
        .map_err(|err| err.to_string())?;
    Ok(pile_id)
}
