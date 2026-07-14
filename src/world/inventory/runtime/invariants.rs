//! World-level inventory invariant checks (ADR-088 I2).

use std::collections::HashSet;

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::InventoryEntryContents;
use super::error::InventoryError;
use super::id::{InventoryId, ItemInstanceId};
use super::ops::resolve_instance_definition;
use super::store::{InventoryStore, ItemInstanceStore};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventoryInvariantReport {
    pub errors: Vec<InventoryError>,
}

impl InventoryInvariantReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn push(&mut self, error: InventoryError) {
        self.errors.push(error);
    }
}

pub fn validate_inventory_stores(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
) -> InventoryInvariantReport {
    let mut report = InventoryInvariantReport::default();
    let mut seen_instances: HashSet<ItemInstanceId> = HashSet::new();

    for inventory_id in inventory_store.sorted_inventory_ids() {
        let Some(record) = inventory_store.get(inventory_id) else {
            report.push(InventoryError::InventoryNotFound(inventory_id));
            continue;
        };
        if let Err(error) =
            record.validate_caches(ctx, |id| resolve_instance_definition(instance_store, id))
        {
            report.push(error);
        }
        for (entry_index, entry) in record.placed_entries().iter().enumerate() {
            if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
                if !seen_instances.insert(*item_instance_id) {
                    report.push(InventoryError::DuplicateItemInstance(*item_instance_id));
                }
                match instance_store.location(*item_instance_id) {
                    Some(crate::world::inventory::ItemInstanceLocation::Inventory {
                        inventory_id: owner,
                        entry_index: idx,
                    }) if owner == inventory_id && idx == entry_index => {}
                    Some(crate::world::inventory::ItemInstanceLocation::Inventory {
                        inventory_id: owner,
                        ..
                    }) => {
                        report.push(InventoryError::UniqueItemAlreadyContained {
                            item_instance_id: *item_instance_id,
                            inventory_id: owner,
                        });
                    }
                    Some(crate::world::inventory::ItemInstanceLocation::WorldPile(_)) => {
                        report.push(InventoryError::ItemInstanceNotFound(*item_instance_id));
                    }
                    None | Some(crate::world::inventory::ItemInstanceLocation::Detached) => {
                        report.push(InventoryError::ItemInstanceNotFound(*item_instance_id))
                    }
                }
            }
        }
    }

    for instance_id in instance_store.sorted_item_instance_ids() {
        if let Some(crate::world::inventory::ItemInstanceLocation::Inventory {
            inventory_id,
            entry_index,
        }) = instance_store.location(instance_id)
        {
            let Some(record) = inventory_store.get(inventory_id) else {
                report.push(InventoryError::InventoryNotFound(inventory_id));
                continue;
            };
            let Some(entry) = record.placed_entries().get(entry_index) else {
                report.push(InventoryError::EntryNotFound {
                    inventory_id,
                    entry_index,
                });
                continue;
            };
            match &entry.contents {
                InventoryEntryContents::Unique {
                    item_instance_id, ..
                } if *item_instance_id == instance_id => {}
                _ => report.push(InventoryError::ItemInstanceNotFound(instance_id)),
            }
        }
    }

    report
}

pub fn assert_inventory_stores(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
) -> Result<(), InventoryError> {
    let report = validate_inventory_stores(inventory_store, instance_store, ctx);
    report.errors.into_iter().next().map_or(Ok(()), Err)
}
