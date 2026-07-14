//! Authoritative world item piles (ADR-090 I4).

mod authoring;
mod error;
mod id;
mod invariants;
mod merge;
mod query;
mod record;
mod settings;
mod store;

#[cfg(test)]
mod tests;

pub use invariants::{
    ItemPileInvariantReport, validate_item_instance_locations, validate_item_pile_store,
};

pub use authoring::{
    DropReport, PickupReport, PileOwnership, SpillReport, drop_stack_from_inventory,
    drop_unique_from_inventory, drop_unit_inventory_entry, pickup_pile_into_inventory,
    spill_inventory_to_world_piles,
};
pub use error::ItemPileError;
pub use id::ItemPileId;
pub use merge::OVERFLOW_PILE_OFFSETS;
pub use query::{item_piles_near, pile_item_definition_id};
pub use record::{ItemPileSource, WorldItemPileRecord, WorldPileContents};
pub use settings::ItemPileSettings;
pub use store::{ChunkItemPileStore, ItemPileStore};
