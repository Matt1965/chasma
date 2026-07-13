/// Starter building definitions for tests and dev fallback when the workbook is absent.
#[cfg(any(test, feature = "dev"))]
mod fixtures {
    use super::super::definition::BuildingDefinition;
    use super::super::definition_id::BuildingDefinitionId;
    use super::super::render_key::BuildingRenderKey;
    use crate::world::building::category::BuildingCategoryId;
    use crate::world::building::footprint::FootprintSpec;

    pub fn starter_definitions() -> Vec<BuildingDefinition> {
        vec![
            BuildingDefinition::new(
                BuildingDefinitionId::new("hut"),
                "Survival Hut",
                BuildingCategoryId::new("residential"),
                BuildingRenderKey::reserved("hut"),
                BuildingRenderKey::reserved("hut_collision"),
                250,
                45.0,
                FootprintSpec::Rectangle {
                    width_meters: 4.0,
                    depth_meters: 4.0,
                },
                35.0,
                true,
            )
            .with_interior_profile_id("two_story_hut"),
            BuildingDefinition::new(
                BuildingDefinitionId::new("workbench"),
                "Workbench",
                BuildingCategoryId::new("production"),
                BuildingRenderKey::reserved("workbench"),
                BuildingRenderKey::reserved("workbench_collision"),
                80,
                0.0,
                FootprintSpec::Rectangle {
                    width_meters: 1.2,
                    depth_meters: 0.8,
                },
                35.0,
                true,
            ),
            BuildingDefinition::new(
                BuildingDefinitionId::new("smelter"),
                "Smelter",
                BuildingCategoryId::new("production"),
                BuildingRenderKey::reserved("smelter"),
                BuildingRenderKey::reserved("smelter_collision"),
                400,
                90.0,
                FootprintSpec::Circle { radius_meters: 2.5 },
                30.0,
                true,
            )
            .with_task_provider_id("smelter_basic"),
        ]
    }
}

#[cfg(any(test, feature = "dev"))]
pub use fixtures::starter_definitions;

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_definitions() -> Vec<super::definition::BuildingDefinition> {
    Vec::new()
}
