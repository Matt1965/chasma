//! Read-only item and inventory profile browser for dev mode (ADR-087 I1).

use crate::world::{
    InventoryProfileCatalog, InventoryProfileDefinition, InventoryProfileId, ItemCatalog,
    ItemCategoryCatalog, ItemDefinition, ItemDefinitionId,
};

use super::dev_mode::{DefinitionId, ItemsBrowserSubtab};

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
    profile_catalog: &InventoryProfileCatalog,
    subtab: ItemsBrowserSubtab,
    search_query: &str,
    enabled_only: bool,
) -> Vec<ItemsBrowserEntry> {
    let query = search_query.trim().to_ascii_lowercase();
    let mut entries = match subtab {
        ItemsBrowserSubtab::Items | ItemsBrowserSubtab::InventoryHarness => {
            item_entries(item_catalog, item_categories, enabled_only)
        }
        ItemsBrowserSubtab::InventoryProfiles => profile_entries(profile_catalog, enabled_only),
    };

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

pub fn format_item_detail(
    item_id: &ItemDefinitionId,
    item_catalog: &ItemCatalog,
    item_categories: &ItemCategoryCatalog,
    unit_catalog: &crate::world::UnitCatalog,
    building_catalog: &crate::world::BuildingCatalog,
) -> String {
    let Some(item) = item_catalog.get(item_id) else {
        return format!("Missing item `{}`", item_id.as_str());
    };
    let category = item_categories
        .get(&item.category_id)
        .map(|cat| cat.display_name.as_str())
        .unwrap_or(item.category_id.as_str());
    let stack = if item.unique_instance_required {
        "unique".to_string()
    } else if item.stackable {
        format!("stackable (max {})", item.max_stack)
    } else {
        "non-stackable".to_string()
    };
    let render = item.render_key.0.as_deref().unwrap_or("-");
    let icon = item.icon_key.0.as_deref().unwrap_or("-");
    let tags = if item.tags.is_empty() {
        "-".to_string()
    } else {
        item.tags.join(", ")
    };
    let unit_refs = units_with_profile_for_item(unit_catalog, item_id);
    let building_refs = buildings_with_profile_for_item(building_catalog, item_id);

    format!(
        "ID: {}\nName: {}\nCategory: {}\nSize: {}x{}\n{stack}\nMass: {} g\nValue: {}\nRender: {}\nIcon: {}\nTags: {}\nEnabled: {}\nUnit profile refs: {}\nBuilding profile refs: {}",
        item.id.as_str(),
        item.display_name,
        category,
        item.grid_width,
        item.grid_height,
        item.mass_grams_per_unit,
        item.base_value_gold,
        render,
        icon,
        tags,
        item.enabled,
        unit_refs,
        building_refs,
    )
}

pub fn format_inventory_profile_detail(
    profile_id: &InventoryProfileId,
    profile_catalog: &InventoryProfileCatalog,
    unit_catalog: &crate::world::UnitCatalog,
    building_catalog: &crate::world::BuildingCatalog,
) -> String {
    let Some(profile) = profile_catalog.get(profile_id) else {
        return format!("Missing profile `{}`", profile_id.as_str());
    };
    let ref_weight = profile
        .reference_weight_grams
        .map(|g| g.to_string())
        .unwrap_or_else(|| "-".to_string());
    let stack_cap = profile
        .global_stack_cap
        .map(|c| c.to_string())
        .unwrap_or_else(|| "-".to_string());
    let unit_refs = units_with_profile(unit_catalog, profile_id);
    let building_refs = buildings_with_profile(building_catalog, profile_id);

    format!(
        "ID: {}\nName: {}\nGrid: {}x{}\nReference weight: {} g (soft)\nGlobal stack cap: {}\nAccess: {:?}\nEnabled: {}\nUnits: {}\nBuildings: {}",
        profile.id.as_str(),
        profile.display_name,
        profile.grid_width,
        profile.grid_height,
        ref_weight,
        stack_cap,
        profile.access_type,
        profile.enabled,
        unit_refs,
        building_refs,
    )
}

fn units_with_profile(
    catalog: &crate::world::UnitCatalog,
    profile_id: &InventoryProfileId,
) -> String {
    let ids: Vec<_> = catalog
        .definitions()
        .iter()
        .filter(|def| {
            def.inventory_profile_id
                .as_ref()
                .is_some_and(|id| id == profile_id)
        })
        .map(|def| def.id.as_str())
        .collect();
    if ids.is_empty() {
        "-".to_string()
    } else {
        ids.join(", ")
    }
}

fn buildings_with_profile(
    catalog: &crate::world::BuildingCatalog,
    profile_id: &InventoryProfileId,
) -> String {
    let ids: Vec<_> = catalog
        .definitions()
        .iter()
        .filter(|def| {
            def.inventory_profile_id
                .as_ref()
                .is_some_and(|id| id == profile_id)
        })
        .map(|def| def.id.as_str())
        .collect();
    if ids.is_empty() {
        "-".to_string()
    } else {
        ids.join(", ")
    }
}

fn units_with_profile_for_item(
    catalog: &crate::world::UnitCatalog,
    _item_id: &ItemDefinitionId,
) -> String {
    let with_inventory = catalog
        .definitions()
        .iter()
        .filter(|def| def.inventory_profile_id.is_some())
        .count();
    if with_inventory == 0 {
        "-".to_string()
    } else {
        format!("{with_inventory} units reference profiles")
    }
}

fn buildings_with_profile_for_item(
    catalog: &crate::world::BuildingCatalog,
    _item_id: &ItemDefinitionId,
) -> String {
    let with_inventory = catalog
        .definitions()
        .iter()
        .filter(|def| def.inventory_profile_id.is_some())
        .count();
    if with_inventory == 0 {
        "-".to_string()
    } else {
        format!("{with_inventory} buildings reference profiles")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog};

    #[test]
    fn items_browser_lists_physical_gold() {
        let entries = filter_items_browser_entries(
            &ItemCatalog::default(),
            &ItemCategoryCatalog::default(),
            &InventoryProfileCatalog::default(),
            ItemsBrowserSubtab::Items,
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
