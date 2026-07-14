use crate::world::{ItemCategoryId, ItemDefinition, ItemDefinitionId, ItemIconKey};

/// Starter item definitions for tests and dev fallback (ADR-087 I1).
pub fn starter_definitions() -> Vec<ItemDefinition> {
    vec![
        ItemDefinition::new(
            ItemDefinitionId::new("gold"),
            "Gold",
            "Physical gold coins carried as stackable currency.",
            ItemCategoryId::new("currency"),
            1,
            1,
            true,
            999,
            1,
            1,
            true,
        )
        .with_icon_key(ItemIconKey::reserved("gold")),
        ItemDefinition::new(
            ItemDefinitionId::new("iron_ore"),
            "Iron Ore",
            "Raw iron ore for smelting.",
            ItemCategoryId::new("raw_material"),
            2,
            2,
            true,
            50,
            2_000,
            2,
            true,
        ),
        ItemDefinition::new(
            ItemDefinitionId::new("healing_kit"),
            "Healing Kit",
            "Medical supplies.",
            ItemCategoryId::new("medicine"),
            2,
            2,
            false,
            1,
            300,
            25,
            true,
        )
        .with_unique_instance_required(true),
    ]
}
