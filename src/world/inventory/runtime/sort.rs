//! Deterministic inventory auto-sort (ADR-088 I2).

use std::cmp::Reverse;

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::{InventoryEntryContents, PlacedInventoryEntry};
use super::error::InventoryError;
use super::grid::{can_place_footprint, first_fit_position, footprint_for_definition};
use super::id::ItemInstanceId;
use super::ops::{rebuild_inventory, resolve_instance_definition};
use super::record::InventoryRecord;
use super::store::ItemInstanceStore;
use crate::world::{ItemCategoryId, ItemDefinitionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SortableEntry {
    contents: InventoryEntryContents,
    definition_id: ItemDefinitionId,
    original_index: usize,
    footprint_area: u32,
    max_dimension: u8,
    category_sort: u32,
    category_id: ItemCategoryId,
    instance_id: Option<ItemInstanceId>,
}

fn merge_compatible_stacks(
    entries: Vec<SortableEntry>,
    ctx: &InventoryCatalogCtx<'_>,
    profile_id: &crate::world::InventoryProfileId,
) -> Result<Vec<SortableEntry>, InventoryError> {
    let mut stacks: Vec<SortableEntry> = Vec::new();
    let mut uniques: Vec<SortableEntry> = Vec::new();

    let mut ordered = entries;
    ordered.sort_by_key(|entry| entry.original_index);

    for entry in ordered {
        match &entry.contents {
            InventoryEntryContents::Unique { .. } => uniques.push(entry),
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } => {
                let item = ctx.require_item(item_definition_id)?;
                let limit = ctx.stack_limit_for(item, profile_id)?;
                if let Some(existing) = stacks
                    .iter_mut()
                    .find(|candidate| candidate.definition_id == *item_definition_id)
                {
                    if let InventoryEntryContents::Stack {
                        quantity: existing_qty,
                        ..
                    } = &mut existing.contents
                    {
                        let room = limit.saturating_sub(*existing_qty);
                        let moved = (*quantity).min(room);
                        *existing_qty = existing_qty
                            .checked_add(moved)
                            .ok_or(InventoryError::QuantityOverflow)?;
                        let remaining = quantity.saturating_sub(moved);
                        if remaining > 0 {
                            stacks.push(SortableEntry {
                                contents: InventoryEntryContents::Stack {
                                    item_definition_id: item_definition_id.clone(),
                                    quantity: remaining,
                                },
                                definition_id: item_definition_id.clone(),
                                original_index: entry.original_index,
                                footprint_area: entry.footprint_area,
                                max_dimension: entry.max_dimension,
                                category_sort: entry.category_sort,
                                category_id: entry.category_id.clone(),
                                instance_id: None,
                            });
                        }
                        continue;
                    }
                }
                stacks.push(entry);
            }
        }
    }

    stacks.append(&mut uniques);
    Ok(stacks)
}

fn sort_entries(mut entries: Vec<SortableEntry>) -> Vec<SortableEntry> {
    entries.sort_by(|a, b| {
        Reverse(a.footprint_area)
            .cmp(&Reverse(b.footprint_area))
            .then_with(|| Reverse(a.max_dimension).cmp(&Reverse(b.max_dimension)))
            .then_with(|| Reverse(a.category_sort).cmp(&Reverse(b.category_sort)))
            .then_with(|| a.category_id.as_str().cmp(b.category_id.as_str()))
            .then_with(|| a.definition_id.as_str().cmp(b.definition_id.as_str()))
            .then_with(|| a.instance_id.cmp(&b.instance_id))
            .then_with(|| a.original_index.cmp(&b.original_index))
    });
    entries
}

fn collect_sortable_entries(
    record: &InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
) -> Result<Vec<SortableEntry>, InventoryError> {
    let mut entries = Vec::new();
    for (original_index, entry) in record.placed_entries().iter().enumerate() {
        let definition_id = match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id, ..
            } => item_definition_id.clone(),
            InventoryEntryContents::Unique { item_instance_id } => {
                resolve_instance_definition(instance_store, *item_instance_id)?
            }
        };
        let item = ctx.require_item(&definition_id)?;
        let (width, height) = footprint_for_definition(item);
        let category_sort = ctx
            .category(&item.category_id)
            .and_then(|category| category.sort_priority)
            .unwrap_or(0);
        let instance_id = match &entry.contents {
            InventoryEntryContents::Unique { item_instance_id } => Some(*item_instance_id),
            InventoryEntryContents::Stack { .. } => None,
        };
        entries.push(SortableEntry {
            contents: entry.contents.clone(),
            definition_id,
            original_index,
            footprint_area: u32::from(width) * u32::from(height),
            max_dimension: width.max(height),
            category_sort,
            category_id: item.category_id.clone(),
            instance_id,
        });
    }
    Ok(entries)
}

/// Deterministic auto-sort with rollback on failure (ADR-088 I2).
pub fn auto_sort_inventory(
    record: &mut InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &mut ItemInstanceStore,
) -> Result<(), InventoryError> {
    let backup = record.clone();
    let result = auto_sort_inventory_inner(record, ctx, instance_store);
    if result.is_err() {
        *record = backup;
    }
    result
}

fn auto_sort_inventory_inner(
    record: &mut InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &mut ItemInstanceStore,
) -> Result<(), InventoryError> {
    let sortable = collect_sortable_entries(record, ctx, instance_store)?;
    let merged = merge_compatible_stacks(sortable, ctx, record.profile_id())?;
    let sorted = sort_entries(merged);

    let mut scratch = InventoryRecord::new(
        record.id(),
        record.owner().clone(),
        record.profile_id().clone(),
        record.grid_width(),
        record.grid_height(),
    );

    let mut placed = Vec::new();
    for entry in sorted {
        let item = ctx.require_item(&entry.definition_id)?;
        let (width, height) = footprint_for_definition(item);
        let (anchor_x, anchor_y) = first_fit_position(&scratch, width, height)?;
        let placed_entry = PlacedInventoryEntry {
            anchor_x,
            anchor_y,
            contents: entry.contents,
        };
        scratch.placed_entries_mut().push(placed_entry.clone());
        rebuild_inventory(&mut scratch, ctx, instance_store)?;
        placed.push(placed_entry);
    }

    record.placed_entries_mut().clear();
    record.placed_entries_mut().extend(placed);
    rebuild_inventory(record, ctx, instance_store)?;

    for (entry_index, entry) in record.placed_entries().iter().enumerate() {
        if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
            instance_store.set_inventory_location(*item_instance_id, record.id(), entry_index);
        }
    }

    Ok(())
}

/// Attempt placement without mutating the source record; used by migration.
pub(crate) fn try_place_entries_in_empty_grid(
    profile_id: &crate::world::InventoryProfileId,
    owner: super::owner::InventoryOwnerRef,
    inventory_id: super::id::InventoryId,
    grid_width: u8,
    grid_height: u8,
    sorted_entries: Vec<SortableEntry>,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
) -> Result<(InventoryRecord, Vec<InventoryEntryContents>), InventoryError> {
    let mut scratch = InventoryRecord::new(
        inventory_id,
        owner,
        profile_id.clone(),
        grid_width,
        grid_height,
    );
    let mut placed = Vec::new();
    let mut leftovers = Vec::new();

    for entry in sorted_entries {
        let item = ctx.require_item(&entry.definition_id)?;
        let (width, height) = footprint_for_definition(item);
        match first_fit_position(&scratch, width, height) {
            Ok((anchor_x, anchor_y)) => {
                let placed_entry = PlacedInventoryEntry {
                    anchor_x,
                    anchor_y,
                    contents: entry.contents,
                };
                scratch.placed_entries_mut().push(placed_entry.clone());
                rebuild_inventory(&mut scratch, ctx, instance_store)?;
                placed.push(placed_entry);
            }
            Err(InventoryError::NoFitPosition { .. }) => leftovers.push(entry.contents),
            Err(error) => return Err(error),
        }
    }

    scratch.placed_entries_mut().clear();
    scratch.placed_entries_mut().extend(placed);
    rebuild_inventory(&mut scratch, ctx, instance_store)?;
    Ok((scratch, leftovers))
}

pub(crate) fn prepare_sorted_entries(
    record: &InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
) -> Result<Vec<SortableEntry>, InventoryError> {
    let sortable = collect_sortable_entries(record, ctx, instance_store)?;
    let merged = merge_compatible_stacks(sortable, ctx, record.profile_id())?;
    Ok(sort_entries(merged))
}
