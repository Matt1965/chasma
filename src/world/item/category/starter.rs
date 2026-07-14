use super::super::category_id::ItemCategoryId;
use super::definition::ItemCategoryDefinition;

/// Starter item categories for tests and dev fallback (ADR-087 I1).
pub fn starter_definitions() -> Vec<ItemCategoryDefinition> {
    vec![
        category("raw_material", "Raw Material", 10),
        category("food", "Food", 20),
        category("weapon", "Weapon", 30),
        category("armor", "Armor", 40),
        category("tool", "Tool", 50),
        category("medicine", "Medicine", 60),
        category("component", "Component", 70),
        category("fuel", "Fuel", 80),
        category("trade_good", "Trade Good", 90),
        category("currency", "Currency", 100),
        category("miscellaneous", "Miscellaneous", 110),
    ]
}

fn category(id: &str, name: &str, sort: u32) -> ItemCategoryDefinition {
    ItemCategoryDefinition::new(ItemCategoryId::new(id), name, "", true).with_sort_priority(sort)
}
