//! Authoritative inventory mutation APIs (ADR-088 I2).

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::{EntryIndex, InventoryEntryContents, PlacedInventoryEntry};
use super::error::InventoryError;
use super::grid::{
    can_place_entry, can_place_footprint, can_place_footprint_excluding, first_fit_position,
    footprint_for_definition, half_stack_quantity, validate_stack_quantity,
};
use super::id::{InventoryId, ItemInstanceId};
use super::instance::{ItemInstance, ItemInstanceMetadata};
use super::owner::InventoryOwnerRef;
use super::record::InventoryRecord;
use super::sort::auto_sort_inventory;
use super::store::{InventoryStore, ItemInstanceStore};
use crate::world::{InventoryProfileId, ItemDefinitionId};

pub fn resolve_instance_definition(
    instance_store: &ItemInstanceStore,
    id: ItemInstanceId,
) -> Result<ItemDefinitionId, InventoryError> {
    instance_store
        .get(id)
        .map(|instance| instance.definition_id.clone())
        .ok_or(InventoryError::ItemInstanceNotFound(id))
}

pub(crate) fn rebuild_inventory(
    record: &mut InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
) -> Result<(), InventoryError> {
    record.rebuild_derived(ctx, |id| resolve_instance_definition(instance_store, id))
}

fn require_inventory_mut<'a>(
    store: &'a mut InventoryStore,
    id: InventoryId,
) -> Result<&'a mut InventoryRecord, InventoryError> {
    store
        .get_mut(id)
        .ok_or(InventoryError::InventoryNotFound(id))
}

fn require_entry<'a>(
    record: &'a InventoryRecord,
    entry_index: EntryIndex,
) -> Result<&'a PlacedInventoryEntry, InventoryError> {
    record
        .placed_entries()
        .get(entry_index)
        .ok_or(InventoryError::EntryNotFound {
            inventory_id: record.id(),
            entry_index,
        })
}

fn definition_for_entry(
    entry: &PlacedInventoryEntry,
    instance_store: &ItemInstanceStore,
) -> Result<ItemDefinitionId, InventoryError> {
    match &entry.contents {
        InventoryEntryContents::Stack {
            item_definition_id, ..
        } => Ok(item_definition_id.clone()),
        InventoryEntryContents::Unique { item_instance_id } => {
            resolve_instance_definition(instance_store, *item_instance_id)
        }
    }
}

fn validate_stack_item(
    ctx: &InventoryCatalogCtx<'_>,
    item_definition_id: &ItemDefinitionId,
    quantity: u32,
    profile_id: &InventoryProfileId,
) -> Result<(), InventoryError> {
    let item = ctx.require_item(item_definition_id)?;
    if item.unique_instance_required || !item.stackable {
        return Err(InventoryError::NonStackableItem(item.id.clone()));
    }
    let limit = ctx.stack_limit_for(item, profile_id)?;
    validate_stack_quantity(item, quantity, limit)
}

fn validate_unique_item(
    ctx: &InventoryCatalogCtx<'_>,
    item_definition_id: &ItemDefinitionId,
) -> Result<(), InventoryError> {
    let item = ctx.require_item(item_definition_id)?;
    if !item.unique_instance_required && item.stackable {
        return Err(InventoryError::UniqueItemRequired(item.id.clone()));
    }
    Ok(())
}

/// Create a detached or future-owned inventory from a profile.
pub fn create_inventory(
    inventory_store: &mut InventoryStore,
    ctx: &InventoryCatalogCtx<'_>,
    profile_id: InventoryProfileId,
    owner: InventoryOwnerRef,
) -> Result<InventoryId, InventoryError> {
    let profile = ctx.require_profile(&profile_id)?;
    if !profile.enabled {
        return Err(InventoryError::ProfileNotFound(profile_id));
    }
    let id = inventory_store.allocate_inventory_id();
    let record = InventoryRecord::new(
        id,
        owner,
        profile_id,
        profile.grid_width,
        profile.grid_height,
    );
    inventory_store.insert(record)?;
    Ok(id)
}

/// Remove a detached inventory and destroy contained unique instances.
pub fn remove_inventory(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    id: InventoryId,
) -> Result<InventoryRecord, InventoryError> {
    let record = inventory_store
        .remove(id)
        .ok_or(InventoryError::InventoryNotFound(id))?;
    if !record.owner().is_detached() {
        return Err(InventoryError::OwnerMismatch {
            inventory_id: id,
            expected: InventoryOwnerRef::Detached,
        });
    }
    for entry in record.placed_entries() {
        if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
            instance_store.clear_location(*item_instance_id);
            let _ = instance_store.remove(*item_instance_id);
        }
    }
    Ok(record)
}

pub fn create_item_instance(
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    definition_id: ItemDefinitionId,
    metadata: ItemInstanceMetadata,
) -> Result<ItemInstanceId, InventoryError> {
    validate_unique_item(ctx, &definition_id)?;
    let id = instance_store.allocate_item_instance_id();
    let instance = ItemInstance::new(id, definition_id).with_metadata(metadata);
    instance_store.insert(instance)?;
    Ok(id)
}

pub fn destroy_item_instance(
    instance_store: &mut ItemInstanceStore,
    id: ItemInstanceId,
) -> Result<ItemInstance, InventoryError> {
    if instance_store.inventory_location(id).is_some() {
        return Err(InventoryError::ItemInstanceUncontainedRequired(id));
    }
    instance_store
        .remove(id)
        .ok_or(InventoryError::ItemInstanceNotFound(id))
}

pub fn place_stack(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_definition_id: ItemDefinitionId,
    quantity: u32,
    anchor_x: u8,
    anchor_y: u8,
) -> Result<EntryIndex, InventoryError> {
    {
        let record = inventory_store
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        validate_stack_item(ctx, &item_definition_id, quantity, record.profile_id())?;
    }
    let entry =
        PlacedInventoryEntry::stack(anchor_x, anchor_y, item_definition_id.clone(), quantity);
    {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        can_place_entry(record, &entry, &item_definition_id, None, ctx)?;
        record.placed_entries_mut().push(entry);
        let entry_index = record.placed_entries().len() - 1;
        rebuild_inventory(record, ctx, instance_store)?;
        Ok(entry_index)
    }
}

pub fn place_stack_first_fit(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_definition_id: ItemDefinitionId,
    quantity: u32,
) -> Result<EntryIndex, InventoryError> {
    let (anchor_x, anchor_y) = {
        let record = inventory_store
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        validate_stack_item(ctx, &item_definition_id, quantity, record.profile_id())?;
        let item = ctx.require_item(&item_definition_id)?;
        let (width, height) = footprint_for_definition(item);
        first_fit_position(record, width, height)?
    };
    place_stack(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        item_definition_id,
        quantity,
        anchor_x,
        anchor_y,
    )
}

pub fn place_unique(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_instance_id: ItemInstanceId,
    anchor_x: u8,
    anchor_y: u8,
) -> Result<EntryIndex, InventoryError> {
    if instance_store
        .inventory_location(item_instance_id)
        .is_some()
    {
        return Err(InventoryError::UniqueItemAlreadyContained {
            item_instance_id,
            inventory_id,
        });
    }
    let definition_id = resolve_instance_definition(instance_store, item_instance_id)?;
    validate_unique_item(ctx, &definition_id)?;
    let entry = PlacedInventoryEntry::unique(anchor_x, anchor_y, item_instance_id);
    {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        can_place_entry(record, &entry, &definition_id, None, ctx)?;
        record.placed_entries_mut().push(entry);
        let entry_index = record.placed_entries().len() - 1;
        instance_store.set_inventory_location(item_instance_id, inventory_id, entry_index);
        rebuild_inventory(record, ctx, instance_store)?;
        Ok(entry_index)
    }
}

pub fn place_unique_first_fit(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_instance_id: ItemInstanceId,
) -> Result<EntryIndex, InventoryError> {
    let (anchor_x, anchor_y) = {
        let record = inventory_store
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        let definition_id = resolve_instance_definition(instance_store, item_instance_id)?;
        let item = ctx.require_item(&definition_id)?;
        let (width, height) = footprint_for_definition(item);
        first_fit_position(record, width, height)?
    };
    place_unique(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        item_instance_id,
        anchor_x,
        anchor_y,
    )
}

pub fn remove_entry(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    entry_index: EntryIndex,
) -> Result<PlacedInventoryEntry, InventoryError> {
    let removed = {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        if entry_index >= record.placed_entries().len() {
            return Err(InventoryError::EntryNotFound {
                inventory_id,
                entry_index,
            });
        }
        record.placed_entries_mut().remove(entry_index)
    };
    if let InventoryEntryContents::Unique { item_instance_id } = &removed.contents {
        instance_store.clear_location(*item_instance_id);
    }
    {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        rebuild_inventory(record, ctx, instance_store)?;
        for (idx, entry) in record.placed_entries().iter().enumerate() {
            if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
                instance_store.set_inventory_location(*item_instance_id, inventory_id, idx);
            }
        }
    }
    Ok(removed)
}

pub fn move_entry(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    entry_index: EntryIndex,
    anchor_x: u8,
    anchor_y: u8,
) -> Result<(), InventoryError> {
    let backup = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();
    let definition_id = {
        let record = inventory_store
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        let entry = require_entry(record, entry_index)?;
        definition_for_entry(entry, instance_store)?
    };
    {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        let entry = require_entry(record, entry_index)?;
        let mut moved = entry.clone();
        moved.anchor_x = anchor_x;
        moved.anchor_y = anchor_y;
        can_place_entry(record, &moved, &definition_id, Some(entry_index), ctx)?;
        record.placed_entries_mut()[entry_index] = moved;
        if let Err(error) = rebuild_inventory(record, ctx, instance_store) {
            *record = backup;
            return Err(error);
        }
    }
    Ok(())
}

pub fn swap_entries(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    entry_a: EntryIndex,
    entry_b: EntryIndex,
) -> Result<(), InventoryError> {
    if entry_a == entry_b {
        return Ok(());
    }
    let backup = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();
    let (def_a, def_b, pos_a, pos_b) = {
        let record = inventory_store
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        let a = require_entry(record, entry_a)?;
        let b = require_entry(record, entry_b)?;
        (
            definition_for_entry(a, instance_store)?,
            definition_for_entry(b, instance_store)?,
            (a.anchor_x, a.anchor_y),
            (b.anchor_x, b.anchor_y),
        )
    };
    {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        let item_a = ctx.require_item(&def_a)?;
        let item_b = ctx.require_item(&def_b)?;
        let (width_a, height_a) = footprint_for_definition(item_a);
        let (width_b, height_b) = footprint_for_definition(item_b);
        if !can_place_footprint_excluding(
            record,
            pos_b.0,
            pos_b.1,
            width_a,
            height_a,
            &[entry_a, entry_b],
        ) || !can_place_footprint_excluding(
            record,
            pos_a.0,
            pos_a.1,
            width_b,
            height_b,
            &[entry_a, entry_b],
        ) {
            return Err(InventoryError::InvalidSwap {
                inventory_id,
                entry_a,
                entry_b,
            });
        }
        let mut entries = record.placed_entries_mut();
        entries[entry_a].anchor_x = pos_b.0;
        entries[entry_a].anchor_y = pos_b.1;
        entries[entry_b].anchor_x = pos_a.0;
        entries[entry_b].anchor_y = pos_a.1;
        if let Err(error) = rebuild_inventory(record, ctx, instance_store) {
            *record = backup;
            return Err(error);
        }
    }
    if let InventoryEntryContents::Unique { item_instance_id } =
        &inventory_store.get(inventory_id).unwrap().placed_entries()[entry_a].contents
    {
        instance_store.set_inventory_location(*item_instance_id, inventory_id, entry_a);
    }
    if let InventoryEntryContents::Unique { item_instance_id } =
        &inventory_store.get(inventory_id).unwrap().placed_entries()[entry_b].contents
    {
        instance_store.set_inventory_location(*item_instance_id, inventory_id, entry_b);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeStacksOutcome {
    pub merged: u32,
    pub remaining_in_source: u32,
}

pub fn merge_stacks(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    source_index: EntryIndex,
    destination_index: EntryIndex,
) -> Result<MergeStacksOutcome, InventoryError> {
    if source_index == destination_index {
        return Ok(MergeStacksOutcome {
            merged: 0,
            remaining_in_source: 0,
        });
    }
    let backup = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();
    let outcome = {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        let profile_id = record.profile_id().clone();
        let source = require_entry(record, source_index)?.clone();
        let destination = require_entry(record, destination_index)?.clone();
        let (
            InventoryEntryContents::Stack {
                item_definition_id: source_item,
                quantity: source_qty,
            },
            InventoryEntryContents::Stack {
                item_definition_id: dest_item,
                quantity: dest_qty,
            },
        ) = (&source.contents, &destination.contents)
        else {
            return Err(InventoryError::CannotMergeUniqueItem);
        };
        if source_item != dest_item {
            return Err(InventoryError::CannotMergeDifferentItems);
        }
        let item = ctx.require_item(source_item)?;
        let limit = ctx.stack_limit_for(item, &profile_id)?;
        let room = limit.saturating_sub(*dest_qty);
        if room == 0 {
            return Ok(MergeStacksOutcome {
                merged: 0,
                remaining_in_source: *source_qty,
            });
        }
        let merged = (*source_qty).min(room);
        if merged == 0 {
            return Ok(MergeStacksOutcome {
                merged: 0,
                remaining_in_source: *source_qty,
            });
        }
        let new_dest_qty = dest_qty
            .checked_add(merged)
            .ok_or(InventoryError::QuantityOverflow)?;
        let remaining = source_qty.saturating_sub(merged);
        record.placed_entries_mut()[destination_index].contents = InventoryEntryContents::Stack {
            item_definition_id: dest_item.clone(),
            quantity: new_dest_qty,
        };
        if remaining == 0 {
            record.placed_entries_mut().remove(source_index);
        } else {
            record.placed_entries_mut()[source_index].contents = InventoryEntryContents::Stack {
                item_definition_id: source_item.clone(),
                quantity: remaining,
            };
        }
        if let Err(error) = rebuild_inventory(record, ctx, instance_store) {
            *record = backup;
            return Err(error);
        }
        MergeStacksOutcome {
            merged,
            remaining_in_source: remaining,
        }
    };
    {
        let record = inventory_store.get(inventory_id).unwrap();
        for (idx, entry) in record.placed_entries().iter().enumerate() {
            if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
                instance_store.set_inventory_location(*item_instance_id, inventory_id, idx);
            }
        }
    }
    Ok(outcome)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitStackOutcome {
    pub moved: u32,
    pub source_remaining: u32,
    pub new_entry_index: EntryIndex,
}

pub fn split_stack(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    source_index: EntryIndex,
    quantity_to_move: u32,
    anchor_x: u8,
    anchor_y: u8,
) -> Result<SplitStackOutcome, InventoryError> {
    let backup = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();
    let outcome = {
        let record = require_inventory_mut(inventory_store, inventory_id)?;
        let source = require_entry(record, source_index)?.clone();
        let InventoryEntryContents::Stack {
            item_definition_id,
            quantity,
        } = &source.contents
        else {
            return Err(InventoryError::NotStackEntry {
                inventory_id,
                entry_index: source_index,
            });
        };
        if quantity_to_move == 0 || quantity_to_move > *quantity {
            return Err(InventoryError::InvalidStackQuantity {
                quantity: quantity_to_move,
                limit: *quantity,
            });
        }
        validate_stack_item(
            ctx,
            item_definition_id,
            quantity_to_move,
            record.profile_id(),
        )?;
        let remaining = quantity
            .checked_sub(quantity_to_move)
            .ok_or(InventoryError::QuantityUnderflow)?;
        let new_entry = PlacedInventoryEntry::stack(
            anchor_x,
            anchor_y,
            item_definition_id.clone(),
            quantity_to_move,
        );
        can_place_entry(
            record,
            &new_entry,
            item_definition_id,
            Some(source_index),
            ctx,
        )?;
        if remaining == 0 {
            record.placed_entries_mut().remove(source_index);
        } else {
            record.placed_entries_mut()[source_index].contents = InventoryEntryContents::Stack {
                item_definition_id: item_definition_id.clone(),
                quantity: remaining,
            };
        }
        record.placed_entries_mut().push(new_entry);
        let new_entry_index = record.placed_entries().len() - 1;
        if let Err(error) = rebuild_inventory(record, ctx, instance_store) {
            *record = backup;
            return Err(error);
        }
        SplitStackOutcome {
            moved: quantity_to_move,
            source_remaining: remaining,
            new_entry_index,
        }
    };
    Ok(outcome)
}

pub fn split_stack_half(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    source_index: EntryIndex,
) -> Result<SplitStackOutcome, InventoryError> {
    let (quantity, item_definition_id) = {
        let record = inventory_store
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        let source = require_entry(record, source_index)?;
        let InventoryEntryContents::Stack {
            item_definition_id,
            quantity,
        } = &source.contents
        else {
            return Err(InventoryError::NotStackEntry {
                inventory_id,
                entry_index: source_index,
            });
        };
        (*quantity, item_definition_id.clone())
    };
    let quantity_to_move = half_stack_quantity(quantity);
    let (anchor_x, anchor_y) = {
        let record = inventory_store
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        let item = ctx.require_item(&item_definition_id)?;
        let (width, height) = footprint_for_definition(item);
        first_fit_position(record, width, height)?
    };
    split_stack(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        source_index,
        quantity_to_move,
        anchor_x,
        anchor_y,
    )
}

pub fn auto_sort(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
) -> Result<(), InventoryError> {
    let record = require_inventory_mut(inventory_store, inventory_id)?;
    auto_sort_inventory(record, ctx, instance_store)
}

pub fn validate_inventory(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
) -> Result<(), InventoryError> {
    let record = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    record.validate_caches(ctx, |id| resolve_instance_definition(instance_store, id))?;
    for entry in record.placed_entries() {
        match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } => {
                validate_stack_item(ctx, item_definition_id, *quantity, record.profile_id())?;
            }
            InventoryEntryContents::Unique { item_instance_id } => {
                let Some((owner_inventory, _)) =
                    instance_store.inventory_location(*item_instance_id)
                else {
                    return Err(InventoryError::ItemInstanceNotFound(*item_instance_id));
                };
                if owner_inventory != inventory_id {
                    return Err(InventoryError::UniqueItemAlreadyContained {
                        item_instance_id: *item_instance_id,
                        inventory_id: owner_inventory,
                    });
                }
            }
        }
    }
    Ok(())
}
