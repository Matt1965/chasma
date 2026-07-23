/// Starter building definitions for tests and dev fallback when the workbook is absent.
#[cfg(any(test, feature = "dev"))]
mod fixtures {
    use bevy::prelude::Vec3;

    use super::super::definition::BuildingDefinition;
    use super::super::definition_id::BuildingDefinitionId;
    use super::super::render_key::BuildingRenderKey;
    use crate::world::operation::OperationDefinitionId;
    use crate::world::InventoryProfileId;
    use crate::world::building::category::BuildingCategoryId;
    use crate::world::building::container_access::ContainerAccessPolicy;
    use crate::world::building::footprint::FootprintSpec;
    use crate::world::building::inventory_binding::{
        BuildingInventoryBindingDefinition, BuildingInventoryBindingId, BuildingInventoryRole,
    };
    use crate::world::logistics::BuildingLogisticsRouteDefinition;
    use crate::world::ItemDefinitionId;

    fn warehouse_route_output(
        local_binding: &str,
        item: &str,
    ) -> BuildingLogisticsRouteDefinition {
        BuildingLogisticsRouteDefinition::output_surplus(
            BuildingInventoryBindingId::new(local_binding),
            ItemDefinitionId::new(item),
            BuildingDefinitionId::new("storage_chest"),
            BuildingInventoryBindingId::new("primary"),
        )
    }

    fn warehouse_route_input(
        local_binding: &str,
        item: &str,
    ) -> BuildingLogisticsRouteDefinition {
        BuildingLogisticsRouteDefinition::input_deficit(
            BuildingInventoryBindingId::new(local_binding),
            ItemDefinitionId::new(item),
            BuildingDefinitionId::new("storage_chest"),
            BuildingInventoryBindingId::new("primary"),
        )
    }
    fn primary_output_binding() -> BuildingInventoryBindingDefinition {
        BuildingInventoryBindingDefinition::new(
            "primary_output",
            BuildingInventoryRole::Output,
            InventoryProfileId::new("chest_large"),
        )
        .with_default(true)
    }

    fn smelter_inventory_bindings() -> Vec<BuildingInventoryBindingDefinition> {
        vec![
            BuildingInventoryBindingDefinition::new(
                "ore_input",
                BuildingInventoryRole::Input,
                InventoryProfileId::new("chest_large"),
            ),
            BuildingInventoryBindingDefinition::new(
                "fuel_input",
                BuildingInventoryRole::Fuel,
                InventoryProfileId::new("chest_small"),
            ),
            BuildingInventoryBindingDefinition::new(
                "metal_output",
                BuildingInventoryRole::Output,
                InventoryProfileId::new("chest_small"),
            ),
            BuildingInventoryBindingDefinition::new(
                "slag_output",
                BuildingInventoryRole::Waste,
                InventoryProfileId::new("chest_small"),
            ),
        ]
    }

    fn bakery_inventory_bindings() -> Vec<BuildingInventoryBindingDefinition> {
        vec![
            BuildingInventoryBindingDefinition::new(
                "flour_input",
                BuildingInventoryRole::Input,
                InventoryProfileId::new("chest_small"),
            ),
            BuildingInventoryBindingDefinition::new(
                "water_input",
                BuildingInventoryRole::Input,
                InventoryProfileId::new("chest_small"),
            ),
            BuildingInventoryBindingDefinition::new(
                "fuel_input",
                BuildingInventoryRole::Fuel,
                InventoryProfileId::new("chest_small"),
            ),
            BuildingInventoryBindingDefinition::new(
                "bread_output",
                BuildingInventoryRole::Output,
                InventoryProfileId::new("chest_small"),
            )
            .with_default(true),
            BuildingInventoryBindingDefinition::new(
                "materials_input",
                BuildingInventoryRole::Input,
                InventoryProfileId::new("chest_small"),
            ),
        ]
    }

    #[allow(dead_code)]
    fn research_desk_inventory_bindings() -> Vec<BuildingInventoryBindingDefinition> {
        vec![
            BuildingInventoryBindingDefinition::new(
                "materials_input",
                BuildingInventoryRole::Input,
                InventoryProfileId::new("chest_small"),
            ),
            BuildingInventoryBindingDefinition::new(
                "general_storage",
                BuildingInventoryRole::General,
                InventoryProfileId::new("chest_small"),
            )
            .with_default(true),
        ]
    }

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
            .with_navigation_blueprint_id("two_story_hut")
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
            )
            .with_supported_operations([
                OperationDefinitionId::new("bake_bread"),
                OperationDefinitionId::new("research"),
            ])
            .with_default_operation_id(OperationDefinitionId::new("bake_bread"))
            .with_inventory_bindings(bakery_inventory_bindings())
            .with_default_inventory_binding_id(BuildingInventoryBindingId::new("bread_output"))
            .with_logistics_routes([
                warehouse_route_input("flour_input", "flour"),
                warehouse_route_input("water_input", "water"),
                warehouse_route_output("bread_output", "bread"),
            ]),
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
            .with_supported_operations([OperationDefinitionId::new("smelt_iron")])
            .with_default_operation_id(OperationDefinitionId::new("smelt_iron"))
            .with_inventory_bindings(smelter_inventory_bindings())
            .with_logistics_routes([
                warehouse_route_input("ore_input", "iron_ore"),
                warehouse_route_output("metal_output", "iron_bar"),
                warehouse_route_output("slag_output", "slag"),
            ]),
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
            .with_navigation_blueprint_id("barn_interior")
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
        .with_supported_operations([OperationDefinitionId::new("mine_iron")])
        .with_default_operation_id(OperationDefinitionId::new("mine_iron"))
        .with_inventory_bindings(vec![primary_output_binding()])
        .with_default_inventory_binding_id(BuildingInventoryBindingId::new("primary_output"))
        .with_logistics_routes([warehouse_route_output("primary_output", "iron_ore")])
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
        .with_supported_operations([OperationDefinitionId::new("mine_stone")])
        .with_default_operation_id(OperationDefinitionId::new("mine_stone"))
        .with_inventory_bindings(vec![primary_output_binding()])
        .with_default_inventory_binding_id(BuildingInventoryBindingId::new("primary_output"))
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
        .with_supported_operations([OperationDefinitionId::new("grow_prispods")])
        .with_default_operation_id(OperationDefinitionId::new("grow_prispods"))
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
        .with_supported_operations([OperationDefinitionId::new("pump_water")])
        .with_default_operation_id(OperationDefinitionId::new("pump_water"))
        .with_inventory_bindings(vec![primary_output_binding()])
        .with_default_inventory_binding_id(BuildingInventoryBindingId::new("primary_output"))
    }
}

#[cfg(any(test, feature = "dev"))]
pub use fixtures::starter_definitions;

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_definitions() -> Vec<super::definition::BuildingDefinition> {
    Vec::new()
}
