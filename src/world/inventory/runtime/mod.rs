//! Authoritative inventory runtime (ADR-088 I2).

mod catalog_ctx;
mod consume;
mod entry;
mod error;
mod grid;
mod id;
mod instance;
mod instance_location;
mod invariants;
mod migration;
mod ops;
mod owner;
mod ownership;
mod record;
mod sort;
mod store;
mod transfer;
mod weight;

#[cfg(test)]
mod tests;

pub use catalog_ctx::InventoryCatalogCtx;
pub use consume::{
    consume_stack_item, count_physical_gold, count_stack_item, physical_gold_item_id,
};
pub use entry::{EntryIndex, InventoryEntryContents, PlacedInventoryEntry};
pub use error::InventoryError;
pub use grid::{
    can_place_entry, can_place_footprint, cells_for_entry, cells_for_footprint, entry_mass_grams,
    first_fit_position, footprint_for_definition, footprint_for_entry, footprint_in_bounds,
    half_stack_quantity, rebuild_derived_state, validate_inventory_caches, validate_stack_quantity,
};
pub use id::{InventoryId, ItemInstanceId};
pub use instance::{ItemInstance, ItemInstanceMetadata};
pub use instance_location::ItemInstanceLocation;
pub use invariants::{
    InventoryInvariantReport, assert_inventory_stores, validate_inventory_stores,
};
pub use migration::{
    InventoryLeftover, ProfileMigrationResult, migrate_inventory_profile,
    migrate_inventory_profile_with_leftovers,
};
pub(crate) use ops::rebuild_inventory;
pub use ops::{
    MergeStacksOutcome, SplitStackOutcome, auto_sort, create_inventory, create_item_instance,
    destroy_item_instance, merge_stacks, move_entry, place_stack, place_stack_first_fit,
    place_unique, place_unique_first_fit, remove_entry, remove_inventory,
    resolve_instance_definition, split_stack, split_stack_half, swap_entries, validate_inventory,
};
pub use owner::InventoryOwnerRef;
pub use ownership::{
    RemovedInventoryContents, create_unit_inventory, profile_for_unit_definition,
    remove_owned_inventory, transfer_inventory_owner,
};
pub use record::{InventoryRecord, cell_index};
pub use sort::auto_sort_inventory;
pub use store::{InventoryStore, ItemInstanceStore};
pub use transfer::{
    TransferError, TransferPlacementPolicy, TransferReport, TransferStatus, loot_corpse_entry,
    transfer_entry_full, transfer_half, transfer_one, transfer_stack_quantity,
    transfer_unique_item,
};
pub use weight::{InventoryWeightQuery, query_inventory_weight};
