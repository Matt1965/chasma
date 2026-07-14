//! Effective stack-limit rule for future inventory placement (ADR-087 I1).

use crate::world::{ItemCategoryId, ItemDefinition};

use super::profile::InventoryProfileDefinition;

/// Compute the effective per-stack limit for an item in a container context.
///
/// `effective_limit = min(item.max_stack, profile.global_stack_cap?, category_cap?, backpack_cap?)`
pub fn effective_stack_limit(
    item: &ItemDefinition,
    profile: Option<&InventoryProfileDefinition>,
    category_stack_cap: Option<u32>,
    backpack_stack_cap: Option<u32>,
) -> u32 {
    let mut limit = item.max_stack;
    if let Some(cap) = profile.and_then(|p| p.global_stack_cap) {
        limit = limit.min(cap);
    }
    if let Some(cap) = category_stack_cap {
        limit = limit.min(cap);
    }
    if let Some(cap) = backpack_stack_cap {
        limit = limit.min(cap);
    }
    limit
}

/// Reserved seam for future per-category caps on profiles.
pub fn category_stack_cap_for(
    _profile: &InventoryProfileDefinition,
    _category_id: &ItemCategoryId,
) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{InventoryProfileDefinition, InventoryProfileId};
    use crate::world::{ItemCategoryId, ItemDefinition, ItemDefinitionId};

    fn gold_item() -> ItemDefinition {
        ItemDefinition::new(
            ItemDefinitionId::new("gold"),
            "Gold",
            "",
            ItemCategoryId::new("currency"),
            1,
            1,
            true,
            999,
            1,
            1,
            true,
        )
    }

    #[test]
    fn effective_stack_limit_uses_minimum_of_caps() {
        let item = gold_item();
        let profile =
            InventoryProfileDefinition::new(InventoryProfileId::new("chest"), "Chest", 8, 8, true)
                .with_global_stack_cap(100);
        assert_eq!(
            effective_stack_limit(&item, Some(&profile), None, None),
            100
        );
        assert_eq!(
            effective_stack_limit(&item, Some(&profile), Some(50), None),
            50
        );
        assert_eq!(
            effective_stack_limit(&item, Some(&profile), Some(50), Some(25)),
            25
        );
    }
}
