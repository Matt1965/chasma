/// Starter building categories for tests and dev fallback.
#[cfg(any(test, feature = "dev"))]
mod fixtures {
    use super::super::definition::BuildingCategoryDefinition;
    use super::super::definition_id::BuildingCategoryId;

    pub fn starter_definitions() -> Vec<BuildingCategoryDefinition> {
        vec![
            BuildingCategoryDefinition::new(
                BuildingCategoryId::new("residential"),
                "Residential",
                "Shelters and housing structures",
                true,
            ),
            BuildingCategoryDefinition::new(
                BuildingCategoryId::new("production"),
                "Production",
                "Crafting, refining, and task-generating structures",
                true,
            ),
            BuildingCategoryDefinition::new(
                BuildingCategoryId::new("defense"),
                "Defense",
                "Walls, towers, and fortifications",
                true,
            ),
        ]
    }
}

#[cfg(any(test, feature = "dev"))]
pub use fixtures::starter_definitions;

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_definitions() -> Vec<super::definition::BuildingCategoryDefinition> {
    Vec::new()
}
