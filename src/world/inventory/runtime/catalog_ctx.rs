use crate::world::{
    InventoryProfileCatalog, InventoryProfileDefinition, InventoryProfileId, ItemCatalog,
    ItemCategoryCatalog, ItemCategoryId, ItemDefinition, ItemDefinitionId,
};

use super::super::stack_limit::{category_stack_cap_for, effective_stack_limit};
use crate::world::item::ItemCategoryDefinition;

/// Catalog access for inventory validation and mutation (ADR-088 I2).
pub struct InventoryCatalogCtx<'a> {
    pub items: &'a ItemCatalog,
    pub categories: &'a ItemCategoryCatalog,
    pub profiles: &'a InventoryProfileCatalog,
}

impl<'a> InventoryCatalogCtx<'a> {
    pub fn new(
        items: &'a ItemCatalog,
        categories: &'a ItemCategoryCatalog,
        profiles: &'a InventoryProfileCatalog,
    ) -> Self {
        Self {
            items,
            categories,
            profiles,
        }
    }

    pub fn item(&self, id: &ItemDefinitionId) -> Option<&ItemDefinition> {
        self.items.get(id)
    }

    pub fn require_item(
        &self,
        id: &ItemDefinitionId,
    ) -> Result<&ItemDefinition, super::error::InventoryError> {
        let Some(def) = self.items.get(id) else {
            return Err(super::error::InventoryError::ItemDefinitionNotFound(
                id.clone(),
            ));
        };
        if !def.enabled {
            return Err(super::error::InventoryError::ItemDefinitionDisabled(
                id.clone(),
            ));
        }
        Ok(def)
    }

    pub fn profile(&self, id: &InventoryProfileId) -> Option<&InventoryProfileDefinition> {
        self.profiles.get(id)
    }

    pub fn require_profile(
        &self,
        id: &InventoryProfileId,
    ) -> Result<&InventoryProfileDefinition, super::error::InventoryError> {
        self.profiles
            .get(id)
            .ok_or_else(|| super::error::InventoryError::ProfileNotFound(id.clone()))
    }

    pub fn category(&self, id: &ItemCategoryId) -> Option<&ItemCategoryDefinition> {
        self.categories.get(id)
    }

    pub fn stack_limit_for(
        &self,
        item: &ItemDefinition,
        profile_id: &InventoryProfileId,
    ) -> Result<u32, super::error::InventoryError> {
        let profile = self.require_profile(profile_id)?;
        let category_cap = category_stack_cap_for(profile, &item.category_id);
        Ok(effective_stack_limit(
            item,
            Some(profile),
            category_cap,
            None,
        ))
    }
}
