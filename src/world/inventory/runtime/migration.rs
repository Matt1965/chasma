//! Profile migration and oversized-stack repair (ADR-088 I2).

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::InventoryEntryContents;
use super::error::InventoryError;
use super::id::InventoryId;
use super::ops::rebuild_inventory;
use super::record::InventoryRecord;
use super::sort::{prepare_sorted_entries, try_place_entries_in_empty_grid};
use super::store::{InventoryStore, ItemInstanceStore};
use crate::world::InventoryProfileId;

/// Contents that could not fit after profile migration (ADR-088 I2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryLeftover {
    pub contents: InventoryEntryContents,
}

/// Result of an atomic profile migration attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMigrationResult {
    pub inventory_id: InventoryId,
    pub leftovers: Vec<InventoryLeftover>,
}

fn split_oversized_stacks(
    record: &InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    new_profile_id: &InventoryProfileId,
) -> Result<Vec<InventoryEntryContents>, InventoryError> {
    let mut contents = Vec::new();
    for entry in record.placed_entries() {
        match &entry.contents {
            InventoryEntryContents::Unique { item_instance_id } => {
                contents.push(InventoryEntryContents::Unique {
                    item_instance_id: *item_instance_id,
                });
            }
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } => {
                let item = ctx.require_item(item_definition_id)?;
                let limit = ctx.stack_limit_for(item, new_profile_id)?;
                let mut remaining = *quantity;
                while remaining > 0 {
                    let chunk = remaining.min(limit);
                    contents.push(InventoryEntryContents::Stack {
                        item_definition_id: item_definition_id.clone(),
                        quantity: chunk,
                    });
                    remaining = remaining.saturating_sub(chunk);
                }
            }
        }
    }
    Ok(contents)
}

fn pack_for_profile(
    backup: &InventoryRecord,
    new_profile_id: InventoryProfileId,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
) -> Result<(InventoryRecord, Vec<InventoryEntryContents>), InventoryError> {
    let profile = ctx.require_profile(&new_profile_id)?;
    if !profile.enabled {
        return Err(InventoryError::ProfileNotFound(new_profile_id));
    }

    let split_contents = split_oversized_stacks(backup, ctx, &new_profile_id)?;
    let mut scratch = InventoryRecord::new(
        backup.id(),
        backup.owner().clone(),
        new_profile_id.clone(),
        profile.grid_width,
        profile.grid_height,
    );
    scratch
        .placed_entries_mut()
        .extend(
            split_contents
                .into_iter()
                .map(|contents| super::entry::PlacedInventoryEntry {
                    anchor_x: 0,
                    anchor_y: 0,
                    contents,
                }),
        );
    rebuild_inventory(&mut scratch, ctx, instance_store)?;

    let sorted = prepare_sorted_entries(&scratch, ctx, instance_store)?;
    try_place_entries_in_empty_grid(
        &new_profile_id,
        backup.owner().clone(),
        backup.id(),
        profile.grid_width,
        profile.grid_height,
        sorted,
        ctx,
        instance_store,
    )
}

/// Repair inventory after profile/grid/stack-cap changes without deleting items.
///
/// Fails atomically when any entry cannot fit. Spill behavior for leftovers is
/// owned by later phases (I3/I5/I4).
pub fn migrate_inventory_profile(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    new_profile_id: InventoryProfileId,
) -> Result<ProfileMigrationResult, InventoryError> {
    let backup = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();
    let (packed, leftover_contents) =
        pack_for_profile(&backup, new_profile_id.clone(), ctx, instance_store)?;

    if !leftover_contents.is_empty() {
        return Err(InventoryError::InvalidProfileMigration {
            inventory_id,
            message: format!(
                "{} entries could not fit in profile `{}`",
                leftover_contents.len(),
                new_profile_id.as_str()
            ),
        });
    }

    let record = require_inventory_mut(inventory_store, inventory_id)?;
    *record = packed;
    rebuild_inventory(record, ctx, instance_store)?;
    Ok(ProfileMigrationResult {
        inventory_id,
        leftovers: Vec::new(),
    })
}

/// Migration that keeps packed entries and returns explicit leftovers.
pub fn migrate_inventory_profile_with_leftovers(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    new_profile_id: InventoryProfileId,
) -> Result<ProfileMigrationResult, InventoryError> {
    let backup = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();
    let (packed, leftover_contents) =
        pack_for_profile(&backup, new_profile_id, ctx, instance_store)?;

    let record = require_inventory_mut(inventory_store, inventory_id)?;
    *record = packed;
    rebuild_inventory(record, ctx, instance_store)?;
    for (entry_index, entry) in record.placed_entries().iter().enumerate() {
        if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
            instance_store.set_inventory_location(*item_instance_id, inventory_id, entry_index);
        }
    }

    Ok(ProfileMigrationResult {
        inventory_id,
        leftovers: leftover_contents
            .into_iter()
            .map(|contents| InventoryLeftover { contents })
            .collect(),
    })
}

fn require_inventory_mut<'a>(
    store: &'a mut InventoryStore,
    id: InventoryId,
) -> Result<&'a mut InventoryRecord, InventoryError> {
    store
        .get_mut(id)
        .ok_or(InventoryError::InventoryNotFound(id))
}
