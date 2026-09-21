use super::definition::ArmorProfileDefinition;
use super::definition_id::ArmorProfileId;

/// Test-only armor profile fixtures. Runtime authority is the design workbook.
pub fn starter_definitions() -> Vec<ArmorProfileDefinition> {
    vec![
        profile("armor_ranger_hood", "Ranger Hood Armor", 5),
        profile("armor_ranger_body", "Ranger Body Armor", 15),
        profile("armor_ranger_arms", "Ranger Arm Armor", 5),
        profile("armor_ranger_legs", "Ranger Leg Armor", 10),
        profile("armor_ranger_feet", "Ranger Foot Armor", 5),
        profile("armor_peasant_body", "Peasant Body Armor", 5),
        profile("armor_peasant_arms", "Peasant Arm Armor", 3),
        profile("armor_peasant_legs", "Peasant Leg Armor", 4),
        profile("armor_peasant_feet", "Peasant Foot Armor", 3),
    ]
}

fn profile(id: &str, name: &str, rating: u32) -> ArmorProfileDefinition {
    ArmorProfileDefinition::new(
        ArmorProfileId::new(id),
        name,
        "Test armor profile fixture.",
        rating,
        true,
    )
}
