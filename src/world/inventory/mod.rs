//! Inventory profile data layer (ADR-087 I1) and authoritative runtime (ADR-088 I2).

mod access;
mod catalog;
mod profile;
mod profile_id;
pub mod runtime;
mod stack_limit;
#[cfg(test)]
mod stress;
mod validation;
mod world_validation;

pub use access::InventoryAccessType;
#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_definitions;
pub use catalog::{InventoryProfileCatalog, InventoryProfileCatalogError};
pub use profile::InventoryProfileDefinition;
pub use profile_id::InventoryProfileId;
pub(crate) use runtime::rebuild_inventory;
pub use runtime::{
    EntryIndex, InventoryCatalogCtx, InventoryEntryContents, InventoryError, InventoryId,
    InventoryInvariantReport, InventoryLeftover, InventoryOwnerRef, InventoryRecord,
    InventoryStore, InventoryWeightQuery, ItemInstance, ItemInstanceId, ItemInstanceLocation,
    ItemInstanceMetadata, ItemInstanceStore, MergeStacksOutcome, PlacedInventoryEntry,
    ProfileMigrationResult, RemovedInventoryContents, SplitStackOutcome, TransferError,
    TransferPlacementPolicy, TransferReport, TransferStatus, assert_inventory_stores, auto_sort,
    auto_sort_inventory, can_place_entry, can_place_footprint, consume_stack_item,
    count_physical_gold, count_stack_item, create_inventory, create_item_instance, create_unit_inventory,
    destroy_item_instance, first_fit_position, half_stack_quantity, loot_corpse_entry, merge_stacks,
    migrate_inventory_profile, migrate_inventory_profile_with_leftovers, move_entry,
    physical_gold_item_id, place_stack, place_stack_first_fit, place_unique,
    place_unique_first_fit, query_inventory_weight, remove_entry, remove_inventory,
    remove_owned_inventory, resolve_instance_definition, split_stack, split_stack_half,
    swap_entries, transfer_entry_full, transfer_half, transfer_inventory_owner, transfer_one,
    transfer_stack_quantity, transfer_unique_item, validate_inventory, validate_inventory_stores,
};
pub use stack_limit::{category_stack_cap_for, effective_stack_limit};
pub use validation::{
    InventoryProfileValidationError, MAX_INVENTORY_GRID_DIMENSION,
    reference_weight_is_soft_encumbrance, validate_inventory_profile,
};
pub use world_validation::{
    WorldInventoryValidationReport, rebuild_all_inventory_derived, validate_world_inventory_state,
};
