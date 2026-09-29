//! Test-only building definitions that are not part of the dev/runtime catalog.

use super::definition::BuildingDefinition;
use super::definition_id::BuildingDefinitionId;
use super::render_key::BuildingRenderKey;
use crate::world::InventoryProfileId;
use crate::world::ItemDefinitionId;
use crate::world::building::category::BuildingCategoryId;
use crate::world::building::footprint::FootprintSpec;
use crate::world::building::inventory_binding::{
    BuildingInventoryBindingDefinition, BuildingInventoryBindingId, BuildingInventoryRole,
};
use crate::world::logistics::BuildingLogisticsRouteDefinition;
use crate::world::operation::OperationDefinitionId;

fn warehouse_route_output(local_binding: &str, item: &str) -> BuildingLogisticsRouteDefinition {
    BuildingLogisticsRouteDefinition::output_surplus(
        BuildingInventoryBindingId::new(local_binding),
        ItemDefinitionId::new(item),
        BuildingDefinitionId::new("storage_chest"),
        BuildingInventoryBindingId::new("primary"),
    )
}

fn warehouse_route_input(local_binding: &str, item: &str) -> BuildingLogisticsRouteDefinition {
    BuildingLogisticsRouteDefinition::input_deficit(
        BuildingInventoryBindingId::new(local_binding),
        ItemDefinitionId::new(item),
        BuildingDefinitionId::new("storage_chest"),
        BuildingInventoryBindingId::new("primary"),
    )
}

/// Smelter gameplay bindings without an authored render model (no glTF preload).
pub fn smelter_building_definition_for_tests() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("smelter"),
        "Smelter",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::unset(),
        BuildingRenderKey::unset(),
        400,
        90.0,
        FootprintSpec::Circle { radius_meters: 2.5 },
        30.0,
        true,
    )
    .with_supported_operations([OperationDefinitionId::new("smelt_iron")])
    .with_default_operation_id(OperationDefinitionId::new("smelt_iron"))
    .with_inventory_bindings(vec![
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
    ])
    .with_logistics_routes([
        warehouse_route_input("ore_input", "iron_ore"),
        warehouse_route_output("metal_output", "iron_bar"),
        warehouse_route_output("slag_output", "slag"),
    ])
}

/// Starter catalog plus the test-only smelter definition (no render preload).
pub fn starter_building_catalog_with_smelter() -> super::registry::BuildingCatalog {
    let categories = crate::world::BuildingCategoryCatalog::default();
    let definitions: Vec<BuildingDefinition> = {
        #[cfg(any(test, feature = "dev"))]
        {
            super::starter::starter_definitions()
                .into_iter()
                .chain([smelter_building_definition_for_tests()])
                .collect()
        }
        #[cfg(not(any(test, feature = "dev")))]
        {
            vec![smelter_building_definition_for_tests()]
        }
    };
    super::registry::BuildingCatalog::from_definitions(definitions, &categories).unwrap()
}
