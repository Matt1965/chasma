//! Per-building inbound storage acceptance policy.

use std::collections::HashSet;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::ItemCategoryId;

/// Authoritative inbound storage filter for storage-capable buildings.
///
/// Empty [`Self::denied_categories`] means all current and future categories are accepted.
#[derive(Debug, Clone, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub struct BuildingStoragePolicy {
    denied_categories: HashSet<ItemCategoryId>,
}

impl BuildingStoragePolicy {
    pub fn accepts_all(&self) -> bool {
        self.denied_categories.is_empty()
    }

    pub fn denied_categories(&self) -> &HashSet<ItemCategoryId> {
        &self.denied_categories
    }

    pub fn accepts_category(&self, category_id: &ItemCategoryId) -> bool {
        !self.denied_categories.contains(category_id)
    }

    pub fn set_category_accepted(&mut self, category_id: &ItemCategoryId, accepted: bool) {
        if accepted {
            self.denied_categories.remove(category_id);
        } else {
            self.denied_categories.insert(category_id.clone());
        }
    }

    pub fn accept_all_categories(&mut self) {
        self.denied_categories.clear();
    }

    pub fn deny_all_categories(&mut self, category_ids: impl IntoIterator<Item = ItemCategoryId>) {
        self.denied_categories = category_ids.into_iter().collect();
    }
}
