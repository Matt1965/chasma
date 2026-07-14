//! Authoritative cross-inventory transfers (ADR-090 I4).

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::{EntryIndex, InventoryEntryContents, PlacedInventoryEntry};
use super::error::InventoryError;
use super::grid::{
    can_place_footprint, first_fit_position, footprint_for_definition, half_stack_quantity,
    validate_stack_quantity,
};
use super::id::{InventoryId, ItemInstanceId};
use super::instance_location::ItemInstanceLocation;
use super::ops::{rebuild_inventory, resolve_instance_definition};
use super::record::InventoryRecord;
use super::store::{InventoryStore, ItemInstanceStore};
use crate::world::ItemDefinitionId;

/// Where to place transferred items in the destination inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferPlacementPolicy {
    ExactCell { x: u8, y: u8 },
    MergeThenFirstFit,
    FirstFitOnly,
}

/// Outcome status for a transfer request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStatus {
    Full,
    Partial,
    Failed,
}

/// Structured transfer report (ADR-090 I4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferReport {
    pub requested: u32,
    pub moved: u32,
    pub remaining_in_source: u32,
    pub merged_into_destination: u32,
    pub new_destination_entry: Option<EntryIndex>,
    pub new_destination_anchor: Option<(u8, u8)>,
    pub status: TransferStatus,
}

impl TransferReport {
    pub fn failed(requested: u32) -> Self {
        Self {
            requested,
            moved: 0,
            remaining_in_source: requested,
            merged_into_destination: 0,
            new_destination_entry: None,
            new_destination_anchor: None,
            status: TransferStatus::Failed,
        }
    }
}

/// Transfer-layer errors (ADR-090 I4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferError {
    SourceInventoryNotFound(InventoryId),
    DestinationInventoryNotFound(InventoryId),
    SourceEntryMissing {
        inventory_id: InventoryId,
        entry_index: EntryIndex,
    },
    InvalidTransferQuantity {
        requested: u32,
        available: u32,
    },
    DestinationNoFit,
    TransferPartialNotAllowed {
        requested: u32,
        movable: u32,
    },
    ItemInstanceLocationMismatch {
        item_instance_id: ItemInstanceId,
    },
    QuantityOverflow,
    Inventory(InventoryError),
}

impl From<InventoryError> for TransferError {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl std::fmt::Display for TransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceInventoryNotFound(id) => write!(f, "source inventory not found `{id:?}`"),
            Self::DestinationInventoryNotFound(id) => {
                write!(f, "destination inventory not found `{id:?}`")
            }
            Self::SourceEntryMissing {
                inventory_id,
                entry_index,
            } => write!(
                f,
                "source entry {entry_index} missing in `{inventory_id:?}`"
            ),
            Self::InvalidTransferQuantity {
                requested,
                available,
            } => write!(
                f,
                "invalid transfer quantity {requested} (available {available})"
            ),
            Self::DestinationNoFit => write!(f, "destination has no fit"),
            Self::TransferPartialNotAllowed { requested, movable } => write!(
                f,
                "partial transfer not allowed: requested {requested}, movable {movable}"
            ),
            Self::ItemInstanceLocationMismatch { item_instance_id } => {
                write!(f, "item instance location mismatch `{item_instance_id:?}`")
            }
            Self::QuantityOverflow => write!(f, "quantity overflow"),
            Self::Inventory(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TransferError {}

fn require_inventory<'a>(
    store: &'a InventoryStore,
    id: InventoryId,
    is_source: bool,
) -> Result<&'a InventoryRecord, TransferError> {
    store.get(id).ok_or(if is_source {
        TransferError::SourceInventoryNotFound(id)
    } else {
        TransferError::DestinationInventoryNotFound(id)
    })
}

fn sync_unique_locations(inventory_store: &InventoryStore, instance_store: &mut ItemInstanceStore) {
    for inventory_id in inventory_store.sorted_inventory_ids() {
        let Some(record) = inventory_store.get(inventory_id) else {
            continue;
        };
        for (idx, entry) in record.placed_entries().iter().enumerate() {
            if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
                instance_store.set_inventory_location(*item_instance_id, inventory_id, idx);
            }
        }
    }
}

/// Transfer a stack quantity between two inventories atomically.
pub fn transfer_stack_quantity(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    destination_inventory_id: InventoryId,
    quantity: u32,
    policy: TransferPlacementPolicy,
    allow_partial: bool,
) -> Result<TransferReport, TransferError> {
    if quantity == 0 {
        return Err(TransferError::InvalidTransferQuantity {
            requested: 0,
            available: 0,
        });
    }
    if source_inventory_id == destination_inventory_id {
        return Err(TransferError::Inventory(
            InventoryError::CannotMergeDifferentItems,
        ));
    }

    let source_backup = require_inventory(inventory_store, source_inventory_id, true)?.clone();
    let dest_backup = require_inventory(inventory_store, destination_inventory_id, false)?.clone();

    let (item_definition_id, available) = {
        let source = require_inventory(inventory_store, source_inventory_id, true)?;
        let entry = source.placed_entries().get(source_entry_index).ok_or(
            TransferError::SourceEntryMissing {
                inventory_id: source_inventory_id,
                entry_index: source_entry_index,
            },
        )?;
        match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity: qty,
            } => (item_definition_id.clone(), *qty),
            _ => {
                return Err(TransferError::Inventory(InventoryError::NotStackEntry {
                    inventory_id: source_inventory_id,
                    entry_index: source_entry_index,
                }));
            }
        }
    };

    if quantity > available {
        return Err(TransferError::InvalidTransferQuantity {
            requested: quantity,
            available,
        });
    }

    let item = ctx
        .require_item(&item_definition_id)
        .map_err(TransferError::from)?;
    let dest_profile = {
        let dest = require_inventory(inventory_store, destination_inventory_id, false)?;
        dest.profile_id().clone()
    };
    let stack_limit = ctx
        .stack_limit_for(item, &dest_profile)
        .map_err(TransferError::from)?;

    let mut remaining_to_move = quantity;
    let mut merged_total = 0u32;
    let mut new_dest_entry = None;
    let mut new_dest_anchor = None;

    {
        let dest = inventory_store.get_mut(destination_inventory_id).ok_or(
            TransferError::DestinationInventoryNotFound(destination_inventory_id),
        )?;

        if matches!(policy, TransferPlacementPolicy::MergeThenFirstFit) {
            let indices: Vec<EntryIndex> = (0..dest.placed_entries().len()).collect();
            for dest_index in indices {
                if remaining_to_move == 0 {
                    break;
                }
                let dest_entry = dest.placed_entries()[dest_index].clone();
                let InventoryEntryContents::Stack {
                    item_definition_id: dest_item,
                    quantity: dest_qty,
                } = &dest_entry.contents
                else {
                    continue;
                };
                if dest_item != &item_definition_id {
                    continue;
                }
                let room = stack_limit.saturating_sub(*dest_qty);
                if room == 0 {
                    continue;
                }
                let merge_qty = remaining_to_move.min(room);
                let new_qty = dest_qty
                    .checked_add(merge_qty)
                    .ok_or(TransferError::QuantityOverflow)?;
                dest.placed_entries_mut()[dest_index].contents = InventoryEntryContents::Stack {
                    item_definition_id: dest_item.clone(),
                    quantity: new_qty,
                };
                merged_total = merged_total.saturating_add(merge_qty);
                remaining_to_move -= merge_qty;
            }
        }

        if remaining_to_move > 0 {
            let (anchor_x, anchor_y) = match policy {
                TransferPlacementPolicy::ExactCell { x, y } => (x, y),
                TransferPlacementPolicy::MergeThenFirstFit
                | TransferPlacementPolicy::FirstFitOnly => {
                    let (w, h) = footprint_for_definition(item);
                    first_fit_position(dest, w, h).map_err(|_| TransferError::DestinationNoFit)?
                }
            };
            let (w, h) = footprint_for_definition(item);
            if !can_place_footprint(dest, anchor_x, anchor_y, w, h, None) {
                if merged_total == 0 {
                    *inventory_store.get_mut(source_inventory_id).unwrap() = source_backup;
                    *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
                    return Err(TransferError::DestinationNoFit);
                }
            } else {
                validate_stack_quantity(item, remaining_to_move, stack_limit)
                    .map_err(TransferError::from)?;
                let new_entry = PlacedInventoryEntry::stack(
                    anchor_x,
                    anchor_y,
                    item_definition_id.clone(),
                    remaining_to_move,
                );
                dest.placed_entries_mut().push(new_entry);
                let idx = dest.placed_entries().len() - 1;
                new_dest_entry = Some(idx);
                new_dest_anchor = Some((anchor_x, anchor_y));
                remaining_to_move = 0;
            }
        }

        if let Err(error) = rebuild_inventory(dest, ctx, instance_store) {
            *inventory_store.get_mut(source_inventory_id).unwrap() = source_backup;
            *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
            return Err(error.into());
        }
    }

    let moved = quantity.saturating_sub(remaining_to_move);
    if moved == 0 {
        *inventory_store.get_mut(source_inventory_id).unwrap() = source_backup;
        *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
        return Err(TransferError::DestinationNoFit);
    }
    if !allow_partial && moved < quantity {
        *inventory_store.get_mut(source_inventory_id).unwrap() = source_backup;
        *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
        return Err(TransferError::TransferPartialNotAllowed {
            requested: quantity,
            movable: moved,
        });
    }

    {
        let source = inventory_store
            .get_mut(source_inventory_id)
            .ok_or(TransferError::SourceInventoryNotFound(source_inventory_id))?;
        let source_remaining = available.saturating_sub(moved);
        if source_remaining == 0 {
            source.placed_entries_mut().remove(source_entry_index);
        } else {
            source.placed_entries_mut()[source_entry_index].contents =
                InventoryEntryContents::Stack {
                    item_definition_id: item_definition_id.clone(),
                    quantity: source_remaining,
                };
        }
        if let Err(error) = rebuild_inventory(source, ctx, instance_store) {
            *inventory_store.get_mut(source_inventory_id).unwrap() = source_backup;
            *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
            return Err(error.into());
        }
    }

    sync_unique_locations(inventory_store, instance_store);

    Ok(TransferReport {
        requested: quantity,
        moved,
        remaining_in_source: available.saturating_sub(moved),
        merged_into_destination: merged_total,
        new_destination_entry: new_dest_entry,
        new_destination_anchor: new_dest_anchor,
        status: if moved == quantity {
            TransferStatus::Full
        } else {
            TransferStatus::Partial
        },
    })
}

/// Transfer an entire stack or unique entry from source to destination.
pub fn transfer_entry_full(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    destination_inventory_id: InventoryId,
    policy: TransferPlacementPolicy,
) -> Result<TransferReport, TransferError> {
    let source = require_inventory(inventory_store, source_inventory_id, true)?;
    let entry = source
        .placed_entries()
        .get(source_entry_index)
        .ok_or(TransferError::SourceEntryMissing {
            inventory_id: source_inventory_id,
            entry_index: source_entry_index,
        })?
        .clone();

    match &entry.contents {
        InventoryEntryContents::Stack { quantity, .. } => transfer_stack_quantity(
            inventory_store,
            instance_store,
            ctx,
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            *quantity,
            policy,
            false,
        ),
        InventoryEntryContents::Unique { item_instance_id } => transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            source_inventory_id,
            source_entry_index,
            *item_instance_id,
            destination_inventory_id,
            policy,
        ),
    }
}

/// Transfer exactly one unit from a stack.
pub fn transfer_one(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    destination_inventory_id: InventoryId,
    policy: TransferPlacementPolicy,
) -> Result<TransferReport, TransferError> {
    transfer_stack_quantity(
        inventory_store,
        instance_store,
        ctx,
        source_inventory_id,
        source_entry_index,
        destination_inventory_id,
        1,
        policy,
        false,
    )
}

/// Transfer half a stack (ceil(quantity/2)).
pub fn transfer_half(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    destination_inventory_id: InventoryId,
    policy: TransferPlacementPolicy,
) -> Result<TransferReport, TransferError> {
    let source = require_inventory(inventory_store, source_inventory_id, true)?;
    let entry = source.placed_entries().get(source_entry_index).ok_or(
        TransferError::SourceEntryMissing {
            inventory_id: source_inventory_id,
            entry_index: source_entry_index,
        },
    )?;
    let quantity = match &entry.contents {
        InventoryEntryContents::Stack { quantity, .. } => *quantity,
        _ => {
            return Err(TransferError::Inventory(InventoryError::NotStackEntry {
                inventory_id: source_inventory_id,
                entry_index: source_entry_index,
            }));
        }
    };
    let half = half_stack_quantity(quantity);
    transfer_stack_quantity(
        inventory_store,
        instance_store,
        ctx,
        source_inventory_id,
        source_entry_index,
        destination_inventory_id,
        half,
        policy,
        false,
    )
}

/// Transfer a unique item between inventories.
pub fn transfer_unique_item(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    item_instance_id: ItemInstanceId,
    destination_inventory_id: InventoryId,
    policy: TransferPlacementPolicy,
) -> Result<TransferReport, TransferError> {
    if source_inventory_id == destination_inventory_id {
        return Err(TransferError::Inventory(
            InventoryError::CannotMergeUniqueItem,
        ));
    }

    match instance_store.location(item_instance_id) {
        Some(ItemInstanceLocation::Inventory {
            inventory_id,
            entry_index,
        }) if inventory_id == source_inventory_id && entry_index == source_entry_index => {}
        Some(_) => {
            return Err(TransferError::ItemInstanceLocationMismatch { item_instance_id });
        }
        None | Some(ItemInstanceLocation::Detached) | Some(ItemInstanceLocation::WorldPile(_)) => {
            return Err(TransferError::ItemInstanceLocationMismatch { item_instance_id });
        }
    }

    let source_backup = require_inventory(inventory_store, source_inventory_id, true)?.clone();
    let dest_backup = require_inventory(inventory_store, destination_inventory_id, false)?.clone();

    let definition_id = resolve_instance_definition(instance_store, item_instance_id)
        .map_err(TransferError::from)?;
    let item = ctx
        .require_item(&definition_id)
        .map_err(TransferError::from)?;
    let (w, h) = footprint_for_definition(item);

    let (anchor_x, anchor_y) = match policy {
        TransferPlacementPolicy::ExactCell { x, y } => (x, y),
        TransferPlacementPolicy::MergeThenFirstFit | TransferPlacementPolicy::FirstFitOnly => {
            let dest = require_inventory(inventory_store, destination_inventory_id, false)?;
            first_fit_position(dest, w, h).map_err(|_| TransferError::DestinationNoFit)?
        }
    };

    {
        let dest = inventory_store.get_mut(destination_inventory_id).ok_or(
            TransferError::DestinationInventoryNotFound(destination_inventory_id),
        )?;
        if !can_place_footprint(dest, anchor_x, anchor_y, w, h, None) {
            return Err(TransferError::DestinationNoFit);
        }
        let new_entry = PlacedInventoryEntry::unique(anchor_x, anchor_y, item_instance_id);
        dest.placed_entries_mut().push(new_entry);
        let dest_index = dest.placed_entries().len() - 1;
        if let Err(error) = rebuild_inventory(dest, ctx, instance_store) {
            *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
            return Err(error.into());
        }
        instance_store.set_inventory_location(
            item_instance_id,
            destination_inventory_id,
            dest_index,
        );
    }

    {
        let source = inventory_store
            .get_mut(source_inventory_id)
            .ok_or(TransferError::SourceInventoryNotFound(source_inventory_id))?;
        source.placed_entries_mut().remove(source_entry_index);
        if let Err(error) = rebuild_inventory(source, ctx, instance_store) {
            *inventory_store.get_mut(source_inventory_id).unwrap() = source_backup;
            *inventory_store.get_mut(destination_inventory_id).unwrap() = dest_backup;
            instance_store.set_inventory_location(
                item_instance_id,
                source_inventory_id,
                source_entry_index,
            );
            return Err(error.into());
        }
    }

    sync_unique_locations(inventory_store, instance_store);

    Ok(TransferReport {
        requested: 1,
        moved: 1,
        remaining_in_source: 0,
        merged_into_destination: 0,
        new_destination_entry: None,
        new_destination_anchor: Some((anchor_x, anchor_y)),
        status: TransferStatus::Full,
    })
}

/// Loot a corpse inventory into a unit inventory (same transfer APIs).
pub fn loot_corpse_entry(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    corpse_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    destination_inventory_id: InventoryId,
    quantity: Option<u32>,
    policy: TransferPlacementPolicy,
) -> Result<TransferReport, TransferError> {
    let source = require_inventory(inventory_store, corpse_inventory_id, true)?;
    let entry = source.placed_entries().get(source_entry_index).ok_or(
        TransferError::SourceEntryMissing {
            inventory_id: corpse_inventory_id,
            entry_index: source_entry_index,
        },
    )?;

    match (&entry.contents, quantity) {
        (InventoryEntryContents::Stack { quantity: qty, .. }, Some(requested)) => {
            transfer_stack_quantity(
                inventory_store,
                instance_store,
                ctx,
                corpse_inventory_id,
                source_entry_index,
                destination_inventory_id,
                requested,
                policy,
                false,
            )
        }
        (InventoryEntryContents::Stack { quantity: qty, .. }, None) => transfer_stack_quantity(
            inventory_store,
            instance_store,
            ctx,
            corpse_inventory_id,
            source_entry_index,
            destination_inventory_id,
            *qty,
            policy,
            false,
        ),
        (InventoryEntryContents::Unique { item_instance_id }, None) => transfer_unique_item(
            inventory_store,
            instance_store,
            ctx,
            corpse_inventory_id,
            source_entry_index,
            *item_instance_id,
            destination_inventory_id,
            policy,
        ),
        (InventoryEntryContents::Unique { .. }, Some(_)) => {
            Err(TransferError::InvalidTransferQuantity {
                requested: 1,
                available: 1,
            })
        }
    }
}
