//! Starter operation definitions for tests and dev (EP3).

use crate::world::ItemDefinitionId;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;

use super::category::OperationCategory;
use super::definition::OperationDefinition;
use super::definition_id::OperationDefinitionId;
use super::io::{OperationInputDefinition, OperationOutputDefinition, OperationTerrainRequirementRef};

fn terrain_req(field: &str, minimum_average_percent: u8) -> OperationTerrainRequirementRef {
    OperationTerrainRequirementRef {
        field_id: crate::world::TerrainFieldId::new(field),
        minimum_average_percent,
    }
}

pub fn starter_definitions() -> Vec<OperationDefinition> {
    vec![
        OperationDefinition::new(
            OperationDefinitionId::new("mine_iron"),
            "Mine Iron",
            "Extract iron ore from terrain deposits.",
            OperationCategory::Extraction,
            10_000,
            2,
        )
        .with_requires_collection(true)
        .with_terrain_requirements(vec![terrain_req("iron", 30)])
        .with_outputs(vec![OperationOutputDefinition::Item {
            item_id: ItemDefinitionId::new("iron_ore"),
            quantity: 1,
            destination_binding: Some(BuildingInventoryBindingId::new("primary_output")),
        }]),
        OperationDefinition::new(
            OperationDefinitionId::new("mine_stone"),
            "Mine Stone",
            "Extract stone from a quarry.",
            OperationCategory::Extraction,
            10_000,
            3,
        )
        .with_requires_collection(true)
        .with_terrain_requirements(vec![terrain_req("stone", 25)])
        .with_outputs(vec![OperationOutputDefinition::Item {
            item_id: ItemDefinitionId::new("stone"),
            quantity: 1,
            destination_binding: Some(BuildingInventoryBindingId::new("primary_output")),
        }]),
        OperationDefinition::new(
            OperationDefinitionId::new("pump_water"),
            "Pump Water",
            "Draw water from a well.",
            OperationCategory::Extraction,
            8_000,
            1,
        )
        .with_terrain_requirements(vec![terrain_req("water", 20)])
        .with_outputs(vec![OperationOutputDefinition::Item {
            item_id: ItemDefinitionId::new("water"),
            quantity: 1,
            destination_binding: Some(BuildingInventoryBindingId::new("primary_output")),
        }]),
        OperationDefinition::new(
            OperationDefinitionId::new("smelt_iron"),
            "Smelt Iron",
            "Refine iron ore into iron bars.",
            OperationCategory::Processing,
            12_000,
            1,
        )
        .with_inputs(vec![OperationInputDefinition {
            item_id: ItemDefinitionId::new("iron_ore"),
            quantity: 2,
            source_binding: Some(BuildingInventoryBindingId::new("ore_input")),
        }])
        .with_outputs(vec![
            OperationOutputDefinition::Item {
                item_id: ItemDefinitionId::new("iron_bar"),
                quantity: 1,
                destination_binding: Some(BuildingInventoryBindingId::new("metal_output")),
            },
            OperationOutputDefinition::Item {
                item_id: ItemDefinitionId::new("slag"),
                quantity: 1,
                destination_binding: Some(BuildingInventoryBindingId::new("slag_output")),
            },
        ]),
        OperationDefinition::new(
            OperationDefinitionId::new("bake_bread"),
            "Bake Bread",
            "Bake bread from flour and water.",
            OperationCategory::Crafting,
            10_000,
            2,
        )
        .with_inputs(vec![
            OperationInputDefinition {
                item_id: ItemDefinitionId::new("flour"),
                quantity: 2,
                source_binding: Some(BuildingInventoryBindingId::new("flour_input")),
            },
            OperationInputDefinition {
                item_id: ItemDefinitionId::new("water"),
                quantity: 1,
                source_binding: Some(BuildingInventoryBindingId::new("water_input")),
            },
        ])
        .with_outputs(vec![OperationOutputDefinition::Item {
            item_id: ItemDefinitionId::new("bread"),
            quantity: 1,
            destination_binding: Some(BuildingInventoryBindingId::new("bread_output")),
        }]),
        OperationDefinition::new(
            OperationDefinitionId::new("grow_prispods"),
            "Grow Prispods",
            "Cultivate prispods in prepared soil.",
            OperationCategory::Agriculture,
            9_000,
            2,
        )
        .with_outputs(vec![OperationOutputDefinition::Item {
            item_id: ItemDefinitionId::new("iron_ore"),
            quantity: 1,
            destination_binding: Some(BuildingInventoryBindingId::new("primary_output")),
        }]),
        OperationDefinition::new(
            OperationDefinitionId::new("research"),
            "Research",
            "Advance settlement knowledge.",
            OperationCategory::Research,
            15_000,
            1,
        )
        .with_repeatable(true)
        .with_inputs(vec![OperationInputDefinition {
            item_id: ItemDefinitionId::new("iron_ore"),
            quantity: 1,
            source_binding: Some(BuildingInventoryBindingId::new("materials_input")),
        }])
        .with_outputs(vec![OperationOutputDefinition::Effect(
            super::io::OperationEffectKind::Research {
                topic_id: "general".into(),
            },
        )]),
    ]
}

#[cfg(any(test, feature = "dev"))]
pub fn test_workbench_operation() -> OperationDefinition {
    OperationDefinition::new(
        OperationDefinitionId::new("test_workbench_op"),
        "Test Workbench",
        "Test-only workbench operation.",
        OperationCategory::Crafting,
        10_000,
        1,
    )
}
