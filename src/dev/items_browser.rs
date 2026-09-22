//! Read-only item and inventory profile browser for dev mode (ADR-087 I1).

use crate::world::{
    InventoryProfileCatalog, InventoryProfileDefinition, ItemCatalog, ItemCategoryCatalog,
    ItemDefinition,
};

use super::dev_mode::DefinitionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemsBrowserEntry {
    pub definition: DefinitionId,
    pub label: String,
    pub category: String,
    pub detail_key: String,
    pub enabled: bool,
}

pub fn filter_items_browser_entries(
    item_catalog: &ItemCatalog,
    item_categories: &ItemCategoryCatalog,
    search_query: &str,
    enabled_only: bool,
) -> Vec<ItemsBrowserEntry> {
    let query = search_query.trim().to_ascii_lowercase();
    let mut entries = item_entries(item_catalog, item_categories, enabled_only);

    if !query.is_empty() {
        entries.retain(|entry| {
            entry.label.to_ascii_lowercase().contains(&query)
                || entry.category.to_ascii_lowercase().contains(&query)
                || entry.detail_key.to_ascii_lowercase().contains(&query)
                || entry
                    .definition
                    .id_str()
                    .to_ascii_lowercase()
                    .contains(&query)
        });
    }

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

fn item_entries(
    catalog: &ItemCatalog,
    categories: &ItemCategoryCatalog,
    enabled_only: bool,
) -> Vec<ItemsBrowserEntry> {
    catalog
        .definitions()
        .iter()
        .filter(|def| !enabled_only || def.enabled)
        .map(|def| item_row(def, categories))
        .collect()
}

/// Read-only profile rows for dev inspection (not shown in Items UI after PLACEMENT-2).
#[allow(dead_code)]
fn profile_entries(
    catalog: &InventoryProfileCatalog,
    enabled_only: bool,
) -> Vec<ItemsBrowserEntry> {
    catalog
        .definitions()
        .iter()
        .filter(|def| !enabled_only || def.enabled)
        .map(profile_row)
        .collect()
}

fn item_row(def: &ItemDefinition, categories: &ItemCategoryCatalog) -> ItemsBrowserEntry {
    let category = categories
        .get(&def.category_id)
        .map(|cat| cat.display_name.clone())
        .unwrap_or_else(|| def.category_id.as_str().to_string());
    let stack = if def.unique_instance_required {
        "unique".to_string()
    } else if def.stackable {
        format!("stack x{}", def.max_stack)
    } else {
        "non-stack".to_string()
    };
    ItemsBrowserEntry {
        definition: DefinitionId::Item(def.id.clone()),
        label: def.display_name.clone(),
        category,
        detail_key: format!("{}x{} {stack}", def.grid_width, def.grid_height),
        enabled: def.enabled,
    }
}

fn profile_row(def: &InventoryProfileDefinition) -> ItemsBrowserEntry {
    ItemsBrowserEntry {
        definition: DefinitionId::InventoryProfile(def.id.clone()),
        label: def.display_name.clone(),
        category: format!("{:?}", def.access_type),
        detail_key: format!("{}x{}", def.grid_width, def.grid_height),
        enabled: def.enabled,
    }
}

pub fn items_catalog_browser_entries(
    item_catalog: &ItemCatalog,
    item_categories: &ItemCategoryCatalog,
    search_query: &str,
    enabled_only: bool,
) -> Vec<super::catalog_browser::CatalogBrowserEntry> {
    filter_items_browser_entries(item_catalog, item_categories, search_query, enabled_only)
        .into_iter()
        .map(|entry| super::catalog_browser::CatalogBrowserEntry {
            definition: entry.definition,
            label: entry.label,
            category: entry.category,
            render_key: entry.detail_key,
            enabled: entry.enabled,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ItemCatalog, ItemCategoryCatalog};

    #[test]
    fn items_browser_lists_physical_gold() {
        let entries = filter_items_browser_entries(
            &ItemCatalog::default(),
            &ItemCategoryCatalog::default(),
            "gold",
            true,
        );
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].definition,
            DefinitionId::Item(ref id) if id.as_str() == "gold"
        ));
    }
}
