//! Drop, pickup, and spill authoring APIs (ADR-090 I4).

use super::error::ItemPileError;
use super::id::ItemPileId;
use super::merge::{merge_candidate_order, offset_position, unit_may_access_pile};
use super::record::{ItemPileSource, WorldItemPileRecord, WorldPileContents};
use super::settings::ItemPileSettings;
use crate::world::inventory::runtime::rebuild_inventory;
use crate::world::inventory::{
    EntryIndex, InventoryCatalogCtx, InventoryEntryContents, InventoryId, InventoryStore,
    ItemInstanceLocation, ItemInstanceStore, TransferReport, TransferStatus, remove_entry,
    resolve_instance_definition,
};
use crate::world::ownership::{Affiliation, OwnerId, TeamId};
use crate::world::unit::UnitId;
use crate::world::{ChunkId, ItemDefinitionId, SpaceId, WorldData, WorldPosition};

/// Report for a drop operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DropReport {
    pub removed_from_inventory: u32,
    pub merged_into_existing_piles: u32,
    pub created_pile_ids: Vec<ItemPileId>,
    pub remaining_in_inventory: u32,
}

/// Report for a pickup operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickupReport {
    pub transfer: TransferReport,
    pub pile_removed: bool,
    pub pile_remaining_quantity: Option<u32>,
}

/// Report for inventory spill to world piles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpillReport {
    pub spilled_entries: u32,
    pub created_pile_ids: Vec<ItemPileId>,
    pub merged_into_existing_piles: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PileOwnership {
    pub owner_id: Option<OwnerId>,
    pub team_id: Option<TeamId>,
    pub affiliation: Affiliation,
}

impl PileOwnership {
    pub fn from_unit(record: &crate::world::unit::UnitRecord) -> Self {
        Self {
            owner_id: record.owner_id,
            team_id: record.team_id,
            affiliation: record.affiliation,
        }
    }
}

fn world_pile_stack_limit(item: &crate::world::ItemDefinition) -> u32 {
    item.max_stack
}

/// Drop a stack quantity from an inventory onto the world.
pub fn drop_stack_from_inventory(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    settings: &ItemPileSettings,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    quantity: u32,
    position: WorldPosition,
    space_id: SpaceId,
    ownership: PileOwnership,
    tick: u64,
) -> Result<DropReport, ItemPileError> {
    let (item_definition_id, stack_available) = {
        let inventory_store = world.inventory_store();
        let record = inventory_store.get(source_inventory_id).ok_or(
            ItemPileError::CorpseInventoryMissing {
                inventory_id: source_inventory_id,
            },
        )?;
        let entry = record.placed_entries().get(source_entry_index).ok_or(
            ItemPileError::MergePlanInvalid("source entry missing".into()),
        )?;
        match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity: available,
            } => {
                if quantity == 0 || quantity > *available {
                    return Err(ItemPileError::QuantityOverflow);
                }
                (item_definition_id.clone(), *available)
            }
            _ => {
                return Err(ItemPileError::MergePlanInvalid("not a stack entry".into()));
            }
        }
    };

    let item = ctx
        .require_item(&item_definition_id)
        .map_err(|_| ItemPileError::MergePlanInvalid("item missing".into()))?;
    let stack_limit = world_pile_stack_limit(item);

    let chunk = ChunkId::new(position.chunk);
    let chunk_piles: Vec<WorldItemPileRecord> = world
        .item_pile_store()
        .piles_in_chunk(chunk)
        .iter()
        .cloned()
        .collect();
    let candidates = merge_candidate_order(
        position,
        space_id,
        &item_definition_id,
        &chunk_piles,
        settings,
    );

    let inv_backup = world
        .inventory_store()
        .get(source_inventory_id)
        .ok_or(ItemPileError::CorpseInventoryMissing {
            inventory_id: source_inventory_id,
        })?
        .clone();
    let pile_store_backup = world.item_pile_store().clone();

    let mut remaining = quantity;
    let mut merged_total = 0u32;
    let mut created_pile_ids = Vec::new();
    let mut overflow_index = 0usize;

    for pile_id in candidates {
        if remaining == 0 {
            break;
        }
        let Some(pile) = world.item_pile_store_mut().get_mut(pile_id) else {
            continue;
        };
        if !unit_may_access_pile(
            pile,
            ownership.owner_id,
            ownership.team_id,
            ownership.affiliation,
        ) {
            continue;
        }
        let WorldPileContents::Stack {
            item_definition_id: pile_item,
            quantity: pile_qty,
        } = &mut pile.contents
        else {
            continue;
        };
        if pile_item != &item_definition_id {
            continue;
        }
        let room = stack_limit.saturating_sub(*pile_qty);
        if room == 0 {
            continue;
        }
        let add = remaining.min(room);
        *pile_qty = pile_qty
            .checked_add(add)
            .ok_or(ItemPileError::QuantityOverflow)?;
        merged_total = merged_total.saturating_add(add);
        remaining -= add;
    }

    while remaining > 0 {
        let place_qty = remaining.min(stack_limit);
        let pile_position = offset_position(position, overflow_index);
        overflow_index += 1;
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        let record = WorldItemPileRecord::new_stack(
            pile_id,
            pile_position,
            space_id,
            item_definition_id.clone(),
            place_qty,
            ownership.owner_id,
            ownership.team_id,
            ownership.affiliation,
            ItemPileSource::Dropped,
            tick,
        );
        world
            .item_pile_store_mut()
            .insert(chunk, record)
            .map_err(|_| ItemPileError::ItemPileIdCollision(pile_id))?;
        created_pile_ids.push(pile_id);
        remaining -= place_qty;
    }

    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let record = inventory_store
            .get_mut(source_inventory_id)
            .ok_or(ItemPileError::DropRollbackFailed)?;
        let entry = record
            .placed_entries()
            .get(source_entry_index)
            .ok_or(ItemPileError::DropRollbackFailed)?;
        let available = match &entry.contents {
            InventoryEntryContents::Stack { quantity, .. } => *quantity,
            _ => return Err(ItemPileError::DropRollbackFailed),
        };
        let new_qty = available
            .checked_sub(quantity)
            .ok_or(ItemPileError::QuantityOverflow)?;
        if new_qty == 0 {
            record.placed_entries_mut().remove(source_entry_index);
        } else {
            record.placed_entries_mut()[source_entry_index].contents =
                InventoryEntryContents::Stack {
                    item_definition_id: item_definition_id.clone(),
                    quantity: new_qty,
                };
        }
        if rebuild_inventory(record, ctx, instance_store).is_err() {
            *inventory_store.get_mut(source_inventory_id).unwrap() = inv_backup;
            *world.item_pile_store_mut() = pile_store_backup;
            return Err(ItemPileError::DropRollbackFailed);
        }
    }

    Ok(DropReport {
        removed_from_inventory: quantity,
        merged_into_existing_piles: merged_total,
        created_pile_ids,
        remaining_in_inventory: stack_available.saturating_sub(quantity),
    })
}

/// Drop a unique item from inventory to a world pile.
pub fn drop_unique_from_inventory(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    position: WorldPosition,
    space_id: SpaceId,
    ownership: PileOwnership,
    tick: u64,
) -> Result<DropReport, ItemPileError> {
    let item_instance_id = {
        let inventory_store = world.inventory_store();
        let record = inventory_store.get(source_inventory_id).ok_or(
            ItemPileError::CorpseInventoryMissing {
                inventory_id: source_inventory_id,
            },
        )?;
        let entry = record.placed_entries().get(source_entry_index).ok_or(
            ItemPileError::MergePlanInvalid("source entry missing".into()),
        )?;
        match &entry.contents {
            InventoryEntryContents::Unique { item_instance_id } => *item_instance_id,
            _ => {
                return Err(ItemPileError::MergePlanInvalid("not unique entry".into()));
            }
        }
    };

    match world.item_instance_store().location(item_instance_id) {
        Some(ItemInstanceLocation::Inventory {
            inventory_id,
            entry_index,
        }) if inventory_id == source_inventory_id && entry_index == source_entry_index => {}
        _ => {
            return Err(ItemPileError::ItemInstanceLocationMismatch { item_instance_id });
        }
    }

    let inv_backup = world
        .inventory_store()
        .get(source_inventory_id)
        .ok_or(ItemPileError::DropRollbackFailed)?
        .clone();
    let pile_store_backup = world.item_pile_store().clone();

    let chunk = ChunkId::new(position.chunk);
    let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
    let record = WorldItemPileRecord::new_unique(
        pile_id,
        position,
        space_id,
        item_instance_id,
        ownership.owner_id,
        ownership.team_id,
        ownership.affiliation,
        ItemPileSource::Dropped,
        tick,
    );
    world
        .item_pile_store_mut()
        .insert(chunk, record)
        .map_err(|_| ItemPileError::ItemPileIdCollision(pile_id))?;

    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    if remove_entry(
        inventory_store,
        instance_store,
        ctx,
        source_inventory_id,
        source_entry_index,
    )
    .is_err()
    {
        *inventory_store.get_mut(source_inventory_id).unwrap() = inv_backup;
        *world.item_pile_store_mut() = pile_store_backup;
        return Err(ItemPileError::DropRollbackFailed);
    }
    instance_store.set_world_pile_location(item_instance_id, pile_id);

    Ok(DropReport {
        removed_from_inventory: 1,
        merged_into_existing_piles: 0,
        created_pile_ids: vec![pile_id],
        remaining_in_inventory: 0,
    })
}

/// Pick up a world pile into an inventory.
pub fn pickup_pile_into_inventory(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    pile_id: ItemPileId,
    destination_inventory_id: InventoryId,
    quantity: Option<u32>,
    actor_owner: Option<OwnerId>,
    actor_team: Option<TeamId>,
    actor_affiliation: Affiliation,
) -> Result<PickupReport, ItemPileError> {
    let pile = world
        .item_pile_store()
        .get(pile_id)
        .cloned()
        .ok_or(ItemPileError::ItemPileNotFound(pile_id))?;

    if !unit_may_access_pile(&pile, actor_owner, actor_team, actor_affiliation) {
        return Err(ItemPileError::Unauthorized);
    }

    let pile_backup = pile.clone();
    let dest_backup = world
        .inventory_store()
        .get(destination_inventory_id)
        .cloned()
        .ok_or(ItemPileError::PickupRollbackFailed)?;

    let transfer = match &pile.contents {
        WorldPileContents::Stack {
            item_definition_id,
            quantity: pile_qty,
        } => {
            let requested = quantity.unwrap_or(*pile_qty);
            if requested == 0 || requested > *pile_qty {
                return Err(ItemPileError::QuantityOverflow);
            }
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            add_stack_to_inventory(
                inventory_store,
                instance_store,
                ctx,
                destination_inventory_id,
                item_definition_id,
                requested,
            )?
        }
        WorldPileContents::Unique { item_instance_id } => {
            if quantity.is_some() {
                return Err(ItemPileError::QuantityOverflow);
            }
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            add_unique_from_pile_to_inventory(
                inventory_store,
                instance_store,
                ctx,
                destination_inventory_id,
                *item_instance_id,
            )?
        }
    };

    let (pile_removed, pile_remaining_quantity) = match &pile_backup.contents {
        WorldPileContents::Stack { quantity, .. } => {
            let taken = transfer.moved;
            if taken >= *quantity {
                world.item_pile_store_mut().remove(pile_id);
                (true, None)
            } else if let Some(record) = world.item_pile_store_mut().get_mut(pile_id) {
                if let WorldPileContents::Stack {
                    quantity: ref mut q,
                    ..
                } = record.contents
                {
                    *q = q.saturating_sub(taken);
                }
                (false, record.stack_quantity())
            } else {
                (false, None)
            }
        }
        WorldPileContents::Unique { item_instance_id } => {
            world.item_pile_store_mut().remove(pile_id);
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            if let Some(entry_index) = transfer.new_destination_entry {
                instance_store.set_inventory_location(
                    *item_instance_id,
                    destination_inventory_id,
                    entry_index,
                );
            }
            (true, None)
        }
    };

    if transfer.moved == 0 {
        let (inventory_store, _) = world.inventory_runtime_mut();
        *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
        if world.item_pile_store().get(pile_id).is_none() {
            let chunk = ChunkId::new(pile_backup.placement.chunk);
            let _ = world.item_pile_store_mut().insert(chunk, pile_backup);
        }
        return Err(ItemPileError::PickupRollbackFailed);
    }

    Ok(PickupReport {
        transfer,
        pile_removed,
        pile_remaining_quantity,
    })
}

fn add_stack_to_inventory(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    destination_inventory_id: InventoryId,
    item_definition_id: &ItemDefinitionId,
    quantity: u32,
) -> Result<TransferReport, ItemPileError> {
    use crate::world::inventory::place_stack_first_fit;
    let backup = inventory_store
        .get(destination_inventory_id)
        .ok_or(ItemPileError::PickupRollbackFailed)?
        .clone();
    if place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        destination_inventory_id,
        item_definition_id.clone(),
        quantity,
    )
    .is_err()
    {
        *inventory_store.get_mut(destination_inventory_id).unwrap() = backup;
        return Err(ItemPileError::PickupRollbackFailed);
    }
    Ok(TransferReport {
        requested: quantity,
        moved: quantity,
        remaining_in_source: 0,
        merged_into_destination: 0,
        new_destination_entry: None,
        new_destination_anchor: None,
        status: TransferStatus::Full,
    })
}

fn add_unique_from_pile_to_inventory(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    destination_inventory_id: InventoryId,
    item_instance_id: crate::world::ItemInstanceId,
) -> Result<TransferReport, ItemPileError> {
    use crate::world::inventory::place_unique_first_fit;
    let definition_id = resolve_instance_definition(instance_store, item_instance_id)
        .map_err(|_| ItemPileError::ItemInstanceLocationMismatch { item_instance_id })?;
    let _ = definition_id;
    let backup = inventory_store
        .get(destination_inventory_id)
        .ok_or(ItemPileError::PickupRollbackFailed)?
        .clone();
    let entry_index = place_unique_first_fit(
        inventory_store,
        instance_store,
        ctx,
        destination_inventory_id,
        item_instance_id,
    )
    .map_err(|_| ItemPileError::PickupRollbackFailed)?;
    instance_store.set_inventory_location(item_instance_id, destination_inventory_id, entry_index);
    Ok(TransferReport {
        requested: 1,
        moved: 1,
        remaining_in_source: 0,
        merged_into_destination: 0,
        new_destination_entry: Some(entry_index),
        new_destination_anchor: None,
        status: TransferStatus::Full,
    })
}

/// Spill all inventory entries to world piles at a position.
pub fn spill_inventory_to_world_piles(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    settings: &ItemPileSettings,
    inventory_id: InventoryId,
    position: WorldPosition,
    space_id: SpaceId,
    ownership: PileOwnership,
    tick: u64,
) -> Result<SpillReport, ItemPileError> {
    let contents: Vec<InventoryEntryContents> = world
        .inventory_store()
        .get(inventory_id)
        .ok_or(ItemPileError::CorpseInventoryMissing { inventory_id })?
        .placed_entries()
        .iter()
        .map(|entry| entry.contents.clone())
        .collect();

    let inv_backup = world
        .inventory_store()
        .get(inventory_id)
        .cloned()
        .ok_or(ItemPileError::CorpseInventoryMissing { inventory_id })?;

    {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let record = inventory_store
            .get_mut(inventory_id)
            .ok_or(ItemPileError::CorpseInventoryMissing { inventory_id })?;
        record.placed_entries_mut().clear();
        if rebuild_inventory(record, ctx, instance_store).is_err() {
            *inventory_store.get_mut(inventory_id).unwrap() = inv_backup;
            return Err(ItemPileError::DropRollbackFailed);
        }
    }

    let mut report = SpillReport::default();
    let chunk = ChunkId::new(position.chunk);
    let mut overflow_index = 0usize;

    for content in contents {
        match content {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } => {
                let item = ctx
                    .require_item(&item_definition_id)
                    .map_err(|_| ItemPileError::MergePlanInvalid("item missing".into()))?;
                let stack_limit = world_pile_stack_limit(item);
                let mut remaining = quantity;
                let chunk_piles: Vec<WorldItemPileRecord> = world
                    .item_pile_store()
                    .piles_in_chunk(chunk)
                    .iter()
                    .cloned()
                    .collect();
                let candidates = merge_candidate_order(
                    position,
                    space_id,
                    &item_definition_id,
                    &chunk_piles,
                    settings,
                );
                for pile_id in candidates {
                    if remaining == 0 {
                        break;
                    }
                    let Some(pile) = world.item_pile_store_mut().get_mut(pile_id) else {
                        continue;
                    };
                    let WorldPileContents::Stack {
                        item_definition_id: pile_item,
                        quantity: pile_qty,
                    } = &mut pile.contents
                    else {
                        continue;
                    };
                    if pile_item != &item_definition_id {
                        continue;
                    }
                    let room = stack_limit.saturating_sub(*pile_qty);
                    let add = remaining.min(room);
                    if add == 0 {
                        continue;
                    }
                    *pile_qty = pile_qty
                        .checked_add(add)
                        .ok_or(ItemPileError::QuantityOverflow)?;
                    report.merged_into_existing_piles += add;
                    remaining -= add;
                }
                while remaining > 0 {
                    let place_qty = remaining.min(stack_limit);
                    let pile_position = offset_position(position, overflow_index);
                    overflow_index += 1;
                    let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
                    let record = WorldItemPileRecord::new_stack(
                        pile_id,
                        pile_position,
                        space_id,
                        item_definition_id.clone(),
                        place_qty,
                        ownership.owner_id,
                        ownership.team_id,
                        ownership.affiliation,
                        ItemPileSource::Spilled,
                        tick,
                    );
                    world
                        .item_pile_store_mut()
                        .insert(chunk, record)
                        .map_err(|_| ItemPileError::ItemPileIdCollision(pile_id))?;
                    report.created_pile_ids.push(pile_id);
                    remaining -= place_qty;
                }
                report.spilled_entries += 1;
            }
            InventoryEntryContents::Unique { item_instance_id } => {
                let pile_position = offset_position(position, overflow_index);
                overflow_index += 1;
                let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
                let record = WorldItemPileRecord::new_unique(
                    pile_id,
                    pile_position,
                    space_id,
                    item_instance_id,
                    ownership.owner_id,
                    ownership.team_id,
                    ownership.affiliation,
                    ItemPileSource::Spilled,
                    tick,
                );
                world
                    .item_pile_store_mut()
                    .insert(chunk, record)
                    .map_err(|_| ItemPileError::ItemPileIdCollision(pile_id))?;
                world
                    .item_instance_store_mut()
                    .set_world_pile_location(item_instance_id, pile_id);
                report.created_pile_ids.push(pile_id);
                report.spilled_entries += 1;
            }
        }
    }

    Ok(report)
}

/// Convenience: drop from a unit's inventory at unit placement.
pub fn drop_unit_inventory_entry(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    settings: &ItemPileSettings,
    unit_id: crate::world::UnitId,
    entry_index: EntryIndex,
    quantity: Option<u32>,
    tick: u64,
) -> Result<DropReport, ItemPileError> {
    let unit = world
        .get_unit(unit_id)
        .cloned()
        .ok_or(ItemPileError::Unauthorized)?;
    let inventory_id = unit
        .inventory_id
        .ok_or(ItemPileError::CorpseInventoryMissing {
            inventory_id: InventoryId::INVALID,
        })?;
    let placement = unit.placement.position;
    let space_id = unit.current_space_id;
    let ownership = PileOwnership::from_unit(&unit);

    let record = world.inventory_store();
    let entry = record
        .get(inventory_id)
        .and_then(|r| r.placed_entries().get(entry_index))
        .ok_or(ItemPileError::MergePlanInvalid("entry missing".into()))?;

    match &entry.contents {
        InventoryEntryContents::Stack { quantity: qty, .. } => {
            let drop_qty = quantity.unwrap_or(*qty);
            drop_stack_from_inventory(
                world,
                ctx,
                settings,
                inventory_id,
                entry_index,
                drop_qty,
                placement,
                space_id,
                ownership,
                tick,
            )
        }
        InventoryEntryContents::Unique { .. } => drop_unique_from_inventory(
            world,
            ctx,
            inventory_id,
            entry_index,
            placement,
            space_id,
            ownership,
            tick,
        ),
    }
}
