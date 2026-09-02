//! Merge Rust-authored gameplay extensions onto Excel-imported building visuals.
//!
//! Excel determines which building definitions exist in the live dev catalog.
//! Matching starter definitions may supplement runtime-only fields only — unmatched
//! starters are never appended.

use bevy::prelude::Vec3;

use super::definition::BuildingDefinition;
use super::registry::{BuildingCatalog, BuildingCatalogError};
use super::starter::starter_definitions;
use crate::world::building::category::BuildingCategoryCatalog;

/// Apply gameplay-only fields from a starter extension onto an Excel-imported definition.
pub fn apply_starter_gameplay_extension(
    base: &mut BuildingDefinition,
    extension: &BuildingDefinition,
) {
    if !extension.supported_operations.is_empty() {
        base.supported_operations = extension.supported_operations.clone();
    }
    base.default_operation_id = extension.default_operation_id.clone();
    if !extension.inventory_bindings.is_empty() {
        base.inventory_bindings = extension.inventory_bindings.clone();
    }
    base.default_inventory_binding_id = extension.default_inventory_binding_id.clone();
    base.field_sampling_footprint_id = extension.field_sampling_footprint_id.clone();
    if !extension.logistics_routes.is_empty() {
        base.logistics_routes = extension.logistics_routes.clone();
    }
    if base.interaction_profile_id.is_none() {
        base.interaction_profile_id = extension.interaction_profile_id.clone();
    }
    if base.interior_profile_id.is_none() {
        base.interior_profile_id = extension.interior_profile_id.clone();
    }
    if base.navigation_blueprint_id.is_none() {
        base.navigation_blueprint_id = extension.navigation_blueprint_id.clone();
    }
    if extension.model_local_offset != Vec3::ZERO {
        base.model_local_offset = extension.model_local_offset;
    }
    if extension.model_yaw_correction_degrees != 0.0 {
        base.model_yaw_correction_degrees = extension.model_yaw_correction_degrees;
    }
    if extension.allow_instance_scale {
        base.allow_instance_scale = extension.allow_instance_scale;
    }
    if extension.inventory_profile_id.is_some() && base.inventory_profile_id.is_none() {
        base.inventory_profile_id = extension.inventory_profile_id.clone();
        base.inventory_access_policy = extension.inventory_access_policy;
        base.inventory_interaction_point_key = extension.inventory_interaction_point_key.clone();
        base.spill_on_destroy = extension.spill_on_destroy;
    }
}

/// Supplement Excel-authored definitions with matching starter runtime metadata only.
///
/// Unmatched starter definitions are ignored — they do not enter the live catalog.
pub fn merge_starter_extensions_into_catalog(
    catalog: &mut BuildingCatalog,
    categories: &BuildingCategoryCatalog,
) -> Result<(), BuildingCatalogError> {
    let extensions: std::collections::HashMap<_, _> = starter_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect();

    let mut definitions: Vec<BuildingDefinition> = catalog.definitions().to_vec();
    for definition in &mut definitions {
        if let Some(extension) = extensions.get(&definition.id) {
            apply_starter_gameplay_extension(definition, extension);
        }
    }

    *catalog = BuildingCatalog::from_definitions(definitions, categories)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::FootprintId;
    use crate::world::building::catalog::definition_id::BuildingDefinitionId;
    use crate::world::building::catalog::render_key::BuildingRenderKey;
    use crate::world::building::category::BuildingCategoryId;
    use crate::world::building::footprint::FootprintSpec;
    use crate::world::building::inventory_binding::{
        BuildingInventoryBindingId, effective_inventory_binding_definitions,
    };

    const STARTER_ONLY_IDS: &[&str] = &[
        "copper_mine",
        "iron_mine",
        "settlement_core",
        "water_well",
        "workbench",
    ];

    fn excel_stone_quarry_visual() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("stone_quarry"),
            "Stone Quarry",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("stone_mine"),
            BuildingRenderKey::reserved("stone_mine"),
            450,
            100.0,
            FootprintSpec::Rectangle {
                width_meters: 16.0,
                depth_meters: 12.0,
            },
            30.0,
            true,
        )
    }

    fn excel_prispod_farm_visual() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("prispod_farm"),
            "Prispod Farm",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("prispod_farm"),
            BuildingRenderKey::reserved("prispod_farm"),
            300,
            80.0,
            FootprintSpec::Rectangle {
                width_meters: 26.0,
                depth_meters: 20.0,
            },
            35.0,
            true,
        )
    }

    #[test]
    fn merge_applies_operations_without_overwriting_render_key() {
        let categories = BuildingCategoryCatalog::default();
        let mut catalog =
            BuildingCatalog::from_definitions(vec![excel_stone_quarry_visual()], &categories)
                .unwrap();
        merge_starter_extensions_into_catalog(&mut catalog, &categories).unwrap();
        let merged = catalog
            .get(&BuildingDefinitionId::new("stone_quarry"))
            .unwrap();
        assert_eq!(merged.render_key.0.as_deref(), Some("stone_mine"));
        assert!(
            merged
                .supported_operations
                .iter()
                .any(|op| op.as_str() == "mine_stone")
        );
        assert_eq!(
            merged.default_inventory_binding_id,
            Some(BuildingInventoryBindingId::new("primary_output"))
        );
    }

    #[test]
    fn merge_does_not_append_starter_only_buildings() {
        let categories = BuildingCategoryCatalog::default();
        let mut catalog = BuildingCatalog::from_definitions(vec![], &categories).unwrap();
        merge_starter_extensions_into_catalog(&mut catalog, &categories).unwrap();
        assert!(catalog.is_empty());
        for &id in STARTER_ONLY_IDS {
            assert!(
                catalog.get(&BuildingDefinitionId::new(id)).is_none(),
                "starter-only `{id}` must not enter live catalog"
            );
        }
    }

    #[test]
    fn merge_leaves_multiple_starter_only_ids_absent() {
        let categories = BuildingCategoryCatalog::default();
        let mut catalog = BuildingCatalog::from_definitions(
            vec![excel_stone_quarry_visual(), excel_prispod_farm_visual()],
            &categories,
        )
        .unwrap();
        merge_starter_extensions_into_catalog(&mut catalog, &categories).unwrap();
        assert_eq!(catalog.len(), 2);
        for &id in STARTER_ONLY_IDS {
            assert!(catalog.get(&BuildingDefinitionId::new(id)).is_none());
        }
    }

    #[test]
    fn stone_quarry_retains_production_metadata_after_merge() {
        let categories = BuildingCategoryCatalog::default();
        let mut catalog =
            BuildingCatalog::from_definitions(vec![excel_stone_quarry_visual()], &categories)
                .unwrap();
        merge_starter_extensions_into_catalog(&mut catalog, &categories).unwrap();
        let quarry = catalog
            .get(&BuildingDefinitionId::new("stone_quarry"))
            .unwrap();
        assert!(
            quarry
                .supported_operations
                .iter()
                .any(|op| op.as_str() == "mine_stone")
        );
        assert_eq!(
            quarry.default_inventory_binding_id,
            Some(BuildingInventoryBindingId::new("primary_output"))
        );
        assert_eq!(
            quarry.field_sampling_footprint_id,
            Some(FootprintId::new("quarry_excavation"))
        );
    }

    #[test]
    fn prispod_farm_retains_production_metadata_after_merge() {
        let categories = BuildingCategoryCatalog::default();
        let mut catalog =
            BuildingCatalog::from_definitions(vec![excel_prispod_farm_visual()], &categories)
                .unwrap();
        merge_starter_extensions_into_catalog(&mut catalog, &categories).unwrap();
        let farm = catalog
            .get(&BuildingDefinitionId::new("prispod_farm"))
            .unwrap();
        assert!(
            farm.supported_operations
                .iter()
                .any(|op| op.as_str() == "grow_prispods")
        );
        assert_eq!(
            farm.default_inventory_binding_id,
            Some(BuildingInventoryBindingId::new("primary_output"))
        );
        let binding = effective_inventory_binding_definitions(farm)
            .into_iter()
            .find(|binding| binding.binding_id.as_str() == "primary_output")
            .expect("primary_output");
        assert_eq!(binding.profile_id.as_str(), "farm_output_buffer");
        assert_eq!(
            farm.field_sampling_footprint_id,
            Some(FootprintId::new("farm_cultivation"))
        );
    }
}
