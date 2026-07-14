//! World item pile invariant validation (ADR-090 I4).

use super::id::ItemPileId;
use super::record::WorldPileContents;
use super::store::ItemPileStore;
use crate::world::inventory::{ItemInstanceLocation, ItemInstanceStore};

/// Report from pile / instance location validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemPileInvariantReport {
    pub errors: Vec<String>,
}

impl ItemPileInvariantReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn push(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }
}

/// Validate pile store indexes and record consistency.
pub fn validate_item_pile_store(store: &ItemPileStore) -> ItemPileInvariantReport {
    let mut report = ItemPileInvariantReport::default();
    let mut seen_ids = std::collections::HashSet::new();

    for pile_id in store.sorted_item_pile_ids() {
        if !seen_ids.insert(pile_id) {
            report.push(format!("duplicate item pile id `{pile_id}`"));
        }
        let Some(chunk) = store.pile_chunk(pile_id) else {
            report.push(format!("pile `{pile_id}` missing chunk location index"));
            continue;
        };
        let Some(record) = store.get(pile_id) else {
            report.push(format!("pile `{pile_id}` missing from store"));
            continue;
        };
        if record.id != pile_id {
            report.push(format!(
                "pile record id mismatch: index `{pile_id}`, record `{}`",
                record.id
            ));
        }
        let chunk_piles = store.piles_in_chunk(chunk);
        if !chunk_piles.iter().any(|pile| pile.id == pile_id) {
            report.push(format!(
                "pile `{pile_id}` not listed in chunk `{chunk:?}` store"
            ));
        }
        match &record.contents {
            WorldPileContents::Stack {
                item_definition_id: _,
                quantity,
            } => {
                if *quantity == 0 {
                    report.push(format!("pile `{pile_id}` has zero stack quantity"));
                }
            }
            WorldPileContents::Unique { item_instance_id } => {
                if !item_instance_id.is_valid() {
                    report.push(format!(
                        "pile `{pile_id}` references invalid unique instance"
                    ));
                }
            }
        }
    }

    report
}

/// Validate unique instance locations against inventories and piles.
pub fn validate_item_instance_locations(
    instance_store: &ItemInstanceStore,
    pile_store: &ItemPileStore,
) -> ItemPileInvariantReport {
    let mut report = ItemPileInvariantReport::default();

    for instance_id in instance_store.sorted_item_instance_ids() {
        let Some(location) = instance_store.location(instance_id) else {
            report.push(format!(
                "instance `{instance_id:?}` has no authoritative location"
            ));
            continue;
        };
        match location {
            ItemInstanceLocation::Detached => {}
            ItemInstanceLocation::Inventory {
                inventory_id,
                entry_index,
            } => {
                if !inventory_id.is_valid() {
                    report.push(format!(
                        "instance `{instance_id:?}` has invalid inventory location"
                    ));
                }
                let _ = entry_index;
            }
            ItemInstanceLocation::WorldPile(pile_id) => {
                let Some(pile) = pile_store.get(pile_id) else {
                    report.push(format!(
                        "instance `{instance_id:?}` points to missing pile `{pile_id}`"
                    ));
                    continue;
                };
                match &pile.contents {
                    WorldPileContents::Unique {
                        item_instance_id: pile_instance,
                    } if *pile_instance == instance_id => {}
                    _ => report.push(format!(
                        "instance `{instance_id:?}` pile `{pile_id}` content mismatch"
                    )),
                }
            }
        }
    }

    for pile_id in pile_store.sorted_item_pile_ids() {
        let Some(pile) = pile_store.get(pile_id) else {
            continue;
        };
        let WorldPileContents::Unique { item_instance_id } = &pile.contents else {
            continue;
        };
        match instance_store.location(*item_instance_id) {
            Some(ItemInstanceLocation::WorldPile(located)) if located == pile_id => {}
            other => report.push(format!(
                "pile `{pile_id}` unique `{item_instance_id:?}` location mismatch: {other:?}"
            )),
        }
    }

    report
}
