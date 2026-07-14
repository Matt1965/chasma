use super::super::access::InventoryAccessType;
use super::super::profile::InventoryProfileDefinition;
use super::super::profile_id::InventoryProfileId;

/// Starter inventory profiles for tests and dev fallback (ADR-087 I1).
pub fn starter_definitions() -> Vec<InventoryProfileDefinition> {
    vec![
        profile(
            "unit_backpack_small",
            "Small Backpack",
            4,
            4,
            8_000,
            InventoryAccessType::OwnerOnly,
        ),
        profile(
            "unit_backpack_standard",
            "Standard Backpack",
            6,
            6,
            15_000,
            InventoryAccessType::OwnerOnly,
        ),
        profile(
            "chest_small",
            "Small Chest",
            4,
            4,
            50_000,
            InventoryAccessType::BuildingStorage,
        ),
        profile(
            "chest_large",
            "Large Chest",
            8,
            8,
            200_000,
            InventoryAccessType::BuildingStorage,
        ),
        profile(
            "corpse_default",
            "Corpse Loot",
            4,
            4,
            10_000,
            InventoryAccessType::CorpseLoot,
        ),
    ]
}

fn profile(
    id: &str,
    name: &str,
    width: u8,
    height: u8,
    reference_weight: u32,
    access: InventoryAccessType,
) -> InventoryProfileDefinition {
    InventoryProfileDefinition::new(InventoryProfileId::new(id), name, width, height, true)
        .with_reference_weight_grams(reference_weight)
        .with_access_type(access)
}
