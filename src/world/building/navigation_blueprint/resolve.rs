//! Resolve the effective navigation blueprint for a building instance.

use super::catalog::BuildingNavigationBlueprintCatalog;
use super::definition::{BuildingNavigationBlueprint, BuildingNavigationBlueprintInstanceOverride};
use super::error::BuildingNavigationBlueprintError;
use super::id::BuildingNavigationBlueprintId;
use crate::world::building::catalog::BuildingDefinition;

/// Resolved navigation blueprint source for one building instance.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedBuildingNavigationBlueprint<'a> {
    Catalog(&'a BuildingNavigationBlueprint),
    Inline(BuildingNavigationBlueprint),
}

impl ResolvedBuildingNavigationBlueprint<'_> {
    pub fn blueprint(&self) -> &BuildingNavigationBlueprint {
        match self {
            Self::Catalog(blueprint) => blueprint,
            Self::Inline(blueprint) => blueprint,
        }
    }
}

/// Resolve navigation blueprint: instance override, then asset default.
pub fn resolve_building_navigation_blueprint<'a>(
    definition: &BuildingDefinition,
    catalog: &'a BuildingNavigationBlueprintCatalog,
    instance_override: Option<&BuildingNavigationBlueprintInstanceOverride>,
) -> Result<Option<ResolvedBuildingNavigationBlueprint<'a>>, BuildingNavigationBlueprintError> {
    if let Some(override_data) = instance_override {
        if let Some(inline) = &override_data.inline_blueprint {
            inline.validate()?;
            return Ok(Some(ResolvedBuildingNavigationBlueprint::Inline(
                inline.clone(),
            )));
        }
        if let Some(id) = &override_data.blueprint_id {
            let blueprint = catalog
                .get(id)
                .ok_or_else(|| BuildingNavigationBlueprintError::BlueprintMissing(id.clone()))?;
            if !blueprint.enabled {
                return Err(BuildingNavigationBlueprintError::BlueprintDisabled(
                    id.clone(),
                ));
            }
            return Ok(Some(ResolvedBuildingNavigationBlueprint::Catalog(blueprint)));
        }
    }

    if let Some(asset_id) = definition.navigation_blueprint_id.as_deref() {
        let id = BuildingNavigationBlueprintId::new(asset_id);
        let blueprint = catalog
            .get(&id)
            .ok_or_else(|| BuildingNavigationBlueprintError::BlueprintMissing(id.clone()))?;
        if !blueprint.enabled {
            return Err(BuildingNavigationBlueprintError::BlueprintDisabled(id));
        }
        return Ok(Some(ResolvedBuildingNavigationBlueprint::Catalog(
            blueprint,
        )));
    }

    #[cfg(feature = "data-import")]
    {
        use super::generate::blueprint_id_for_building;
        let generated_id = blueprint_id_for_building(definition);
        if let Some(blueprint) = catalog.get(&generated_id) {
            if !blueprint.enabled {
                return Err(BuildingNavigationBlueprintError::BlueprintDisabled(
                    generated_id,
                ));
            }
            return Ok(Some(ResolvedBuildingNavigationBlueprint::Catalog(
                blueprint,
            )));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::catalog::BuildingDefinitionId;
    use crate::world::starter_building_definitions;
    use crate::world::building::footprint::FootprintSpec;
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;

    #[test]
    fn asset_default_resolves_from_definition() {
        let catalog = BuildingNavigationBlueprintCatalog::default();
        let definition = starter_building_definitions()
            .into_iter()
            .find(|def| def.id == BuildingDefinitionId::new("hut"))
            .expect("hut definition");
        let resolved = resolve_building_navigation_blueprint(
            &definition.with_navigation_blueprint_id("two_story_hut"),
            &catalog,
            None,
        )
        .expect("resolve")
        .expect("blueprint");
        assert_eq!(
            resolved.blueprint().id,
            BuildingNavigationBlueprintId::new("two_story_hut")
        );
    }

    #[test]
    fn inline_override_takes_precedence() {
        let catalog = BuildingNavigationBlueprintCatalog::default();
        let definition = starter_building_definitions()
            .into_iter()
            .find(|def| def.id == BuildingDefinitionId::new("hut"))
            .expect("hut definition");
        let inline = two_story_hut_navigation_blueprint();
        let resolved = resolve_building_navigation_blueprint(
            &definition,
            &catalog,
            Some(&BuildingNavigationBlueprintInstanceOverride::inline(inline)),
        )
        .expect("resolve")
        .expect("blueprint");
        assert_eq!(resolved.blueprint().display_name, "Two Story Hut Navigation");
    }

    #[test]
    fn missing_asset_reference_errors() {
        let catalog = BuildingNavigationBlueprintCatalog::default();
        let definition = BuildingDefinition::new(
            BuildingDefinitionId::new("orphan"),
            "Orphan",
            crate::world::BuildingCategoryId::new("residential"),
            crate::world::BuildingRenderKey::reserved("hut"),
            crate::world::BuildingRenderKey::reserved("hut_collision"),
            100,
            10.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )
        .with_navigation_blueprint_id("missing_blueprint");
        assert!(matches!(
            resolve_building_navigation_blueprint(&definition, &catalog, None),
            Err(BuildingNavigationBlueprintError::BlueprintMissing(_))
        ));
    }
}
