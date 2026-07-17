/// Starter building definitions for tests and dev fallback when the workbook is absent.
#[cfg(any(test, feature = "dev"))]
mod fixtures {
    use bevy::prelude::Vec3;

    use super::super::definition::BuildingDefinition;
    use super::super::definition_id::BuildingDefinitionId;
    use super::super::render_key::BuildingRenderKey;
    use crate::world::InventoryProfileId;
    use crate::world::building::category::BuildingCategoryId;
    use crate::world::building::container_access::ContainerAccessPolicy;
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
            .with_interior_profile_id("two_story_hut")
            .with_allow_instance_scale(true),
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
            BuildingDefinition::new(
                BuildingDefinitionId::new("storage_chest"),
                "Storage Chest",
                BuildingCategoryId::new("storage"),
                BuildingRenderKey::reserved("chest"),
                BuildingRenderKey::reserved("chest"),
                120,
                15.0,
                FootprintSpec::Rectangle {
                    width_meters: 1.0,
                    depth_meters: 0.8,
                },
                35.0,
                true,
            )
            .with_inventory_profile_id(InventoryProfileId::new("chest_small"))
            .with_inventory_access_policy(ContainerAccessPolicy::OwnerOnly)
            .with_inventory_interaction_point_key("access")
            .with_spill_on_destroy(true)
            .with_interaction_profile_id("storage_chest"),
            BuildingDefinition::new(
                BuildingDefinitionId::new("barn"),
                "Barn",
                BuildingCategoryId::new("storage"),
                BuildingRenderKey::reserved("barn"),
                BuildingRenderKey::reserved("barn"),
                400,
                90.0,
                FootprintSpec::Rectangle {
                    width_meters: 8.0,
                    depth_meters: 6.0,
                },
                35.0,
                true,
            )
            .with_inventory_profile_id(InventoryProfileId::new("chest_large"))
            .with_inventory_access_policy(ContainerAccessPolicy::OwnerOnly)
            .with_inventory_interaction_point_key("access")
            .with_spill_on_destroy(true)
            .with_interaction_profile_id("storage_chest")
            .with_interior_profile_id("barn_interior")
            .with_model_local_offset(Vec3::new(7.05, 0.35, -18.65)),
            BuildingDefinition::new(
                BuildingDefinitionId::new("settlement_core"),
                "Settlement Core",
                BuildingCategoryId::new("residential"),
                BuildingRenderKey::reserved("hut"),
                BuildingRenderKey::reserved("hut_collision"),
                500,
                120.0,
                FootprintSpec::Rectangle {
                    width_meters: 6.0,
                    depth_meters: 6.0,
                },
                35.0,
                true,
            )
            .with_interaction_profile_id("settlement_core"),
            tf4_iron_mine(),
            tf4_copper_mine(),
            tf4_stone_quarry(),
            tf4_prispod_farm(),
            tf4_water_well(),
        ]
    }

    fn tf4_iron_mine() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("iron_mine"),
            "Iron Mine",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("smelter"),
            BuildingRenderKey::reserved("smelter_collision"),
            500,
            120.0,
            FootprintSpec::Circle { radius_meters: 3.0 },
            30.0,
            true,
        )
        .with_task_provider_id("iron_mine_basic")
    }

    fn tf4_copper_mine() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("copper_mine"),
            "Copper Mine",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("smelter"),
            BuildingRenderKey::reserved("smelter_collision"),
            500,
            120.0,
            FootprintSpec::Circle { radius_meters: 3.0 },
            30.0,
            true,
        )
        .with_task_provider_id("copper_mine_basic")
    }

    fn tf4_stone_quarry() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("stone_quarry"),
            "Stone Quarry",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("smelter"),
            BuildingRenderKey::reserved("smelter_collision"),
            450,
            100.0,
            FootprintSpec::Rectangle {
                width_meters: 6.0,
                depth_meters: 6.0,
            },
            30.0,
            true,
        )
        .with_field_sampling_footprint_id(crate::world::FootprintId::new("quarry_excavation"))
    }

    fn tf4_prispod_farm() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("prispod_farm"),
            "Prispod Farm",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            300,
            80.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )
        .with_field_sampling_footprint_id(crate::world::FootprintId::new("farm_cultivation"))
    }

    fn tf4_water_well() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("water_well"),
            "Water Well",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("workbench"),
            BuildingRenderKey::reserved("workbench_collision"),
            120,
            30.0,
            FootprintSpec::Circle { radius_meters: 1.0 },
            35.0,
            true,
        )
        .with_field_sampling_footprint_id(crate::world::FootprintId::new("well_extraction"))
    }
}

#[cfg(any(test, feature = "dev"))]
pub use fixtures::starter_definitions;

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_definitions() -> Vec<super::definition::BuildingDefinition> {
    Vec::new()
}
