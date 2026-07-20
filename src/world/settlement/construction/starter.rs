//! Starter construction response mappings and building costs (SA9).

use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::building::category::BuildingCategoryId;
use crate::world::building::operation::OperationDefinitionId;
use crate::world::item::ItemDefinitionId;

use super::catalog::{
    BuildingConstructionCostDefinition, ConstructionCapabilityKind, ConstructionResponseMapping,
};

pub fn starter_construction_mappings() -> Vec<ConstructionResponseMapping> {
    vec![
        ConstructionResponseMapping::new(
            "construct_food_building",
            "Construct Food Building",
            ConstructionCapabilityKind::SupportingOperation(OperationDefinitionId::new(
                "grow_prispods",
            )),
            "food_production",
            1,
            true,
        )
        .with_eligible_buildings([BuildingDefinitionId::new("prispod_farm")]),
        ConstructionResponseMapping::new(
            "construct_housing",
            "Construct Housing",
            ConstructionCapabilityKind::BuildingCategory(BuildingCategoryId::new("residential")),
            "housing",
            1,
            true,
        )
        .with_eligible_buildings([BuildingDefinitionId::new("hut")]),
        ConstructionResponseMapping::new(
            "construct_defenses",
            "Construct Defenses",
            // No dedicated defense buildings yet — allow-list seam for future walls/towers.
            ConstructionCapabilityKind::ExplicitAllowList,
            "defense",
            1,
            true,
        )
        .with_eligible_buildings([BuildingDefinitionId::new("hut")]),
        ConstructionResponseMapping::new(
            "advance_construction",
            "Advance Construction",
            ConstructionCapabilityKind::ExplicitAllowList,
            "advance_construction",
            0,
            false,
        ),
    ]
}

/// Authoritative construction material costs keyed by building definition.
pub fn starter_construction_costs() -> Vec<BuildingConstructionCostDefinition> {
    vec![
        BuildingConstructionCostDefinition {
            building_definition_id: BuildingDefinitionId::new("prispod_farm"),
            materials: vec![(ItemDefinitionId::new("stone"), 10)],
        },
        BuildingConstructionCostDefinition {
            building_definition_id: BuildingDefinitionId::new("hut"),
            materials: vec![(ItemDefinitionId::new("stone"), 5)],
        },
    ]
}
