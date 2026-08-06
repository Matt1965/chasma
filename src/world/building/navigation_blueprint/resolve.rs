//! Resolve the effective navigation blueprint for a building instance.

use super::catalog::BuildingNavigationBlueprintCatalog;
use super::definition::{BuildingNavigationBlueprint, BuildingNavigationBlueprintInstanceOverride};
use super::error::BuildingNavigationBlueprintError;
use super::id::{BuildingNavigationBlueprintId, blueprint_id_for_building};
use super::migrate::migrate_blueprint_to_current;
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
            let mut migrated = inline.clone();
            migrate_blueprint_to_current(&mut migrated)?;
            migrated.validate()?;
            return Ok(Some(ResolvedBuildingNavigationBlueprint::Inline(migrated)));
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
            return Ok(Some(ResolvedBuildingNavigationBlueprint::Catalog(
                blueprint,
            )));
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

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::catalog::BuildingDefinitionId;
    use crate::world::building::footprint::FootprintSpec;
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;
    use crate::world::starter_building_definitions;

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
        assert_eq!(
            resolved.blueprint().display_name,
            "Two Story Hut Navigation"
        );
    }

    /// Definition with neither an interior profile nor an explicit blueprint id —
    /// the real imported Survival Hut shape.
    fn profile_less_definition() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Survival Hut",
            crate::world::BuildingCategoryId::new("residential"),
            crate::world::BuildingRenderKey::reserved("hut"),
            crate::world::BuildingRenderKey::reserved("hut_collision"),
            250,
            45.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )
    }

    fn catalog_with(blueprint: BuildingNavigationBlueprint) -> BuildingNavigationBlueprintCatalog {
        BuildingNavigationBlueprintCatalog::from_definitions(vec![blueprint]).expect("catalog")
    }

    /// IN-11b: the generated-id read path must work in every build configuration.
    /// It used to be `#[cfg(feature = "data-import")]`, which made editor-exported
    /// catalog entries unreachable in builds without the import feature.
    #[test]
    fn generated_id_resolves_without_data_import_feature() {
        let definition = profile_less_definition();
        let mut blueprint = two_story_hut_navigation_blueprint();
        blueprint.id = BuildingNavigationBlueprintId::new("hut_nav");
        let catalog = catalog_with(blueprint);

        let resolved = resolve_building_navigation_blueprint(&definition, &catalog, None)
            .expect("resolve")
            .expect("generated id must resolve regardless of feature flags");
        assert_eq!(
            resolved.blueprint().id,
            BuildingNavigationBlueprintId::new("hut_nav")
        );
    }

    #[test]
    fn generated_id_uses_definition_id_when_no_profile_or_blueprint_named() {
        assert_eq!(
            blueprint_id_for_building(&profile_less_definition()).as_str(),
            "hut_nav"
        );
    }

    #[test]
    fn explicit_definition_blueprint_outranks_generated_id() {
        let definition = profile_less_definition().with_navigation_blueprint_id("two_story_hut");
        let mut generated = two_story_hut_navigation_blueprint();
        generated.id = BuildingNavigationBlueprintId::new("hut_nav");
        generated.display_name = "Generated".to_string();
        let catalog = BuildingNavigationBlueprintCatalog::from_definitions(vec![
            two_story_hut_navigation_blueprint(),
            generated,
        ])
        .expect("catalog");

        let resolved = resolve_building_navigation_blueprint(&definition, &catalog, None)
            .expect("resolve")
            .expect("blueprint");
        assert_eq!(
            resolved.blueprint().id,
            BuildingNavigationBlueprintId::new("two_story_hut")
        );
    }

    #[test]
    fn instance_override_resolves_without_interior_profile() {
        let definition = profile_less_definition();
        let catalog = BuildingNavigationBlueprintCatalog::from_definitions(Vec::new())
            .expect("empty catalog");
        let resolved = resolve_building_navigation_blueprint(
            &definition,
            &catalog,
            Some(&BuildingNavigationBlueprintInstanceOverride::inline(
                two_story_hut_navigation_blueprint(),
            )),
        )
        .expect("resolve")
        .expect("inline override must resolve without a profile");
        assert_eq!(
            resolved.blueprint().display_name,
            "Two Story Hut Navigation"
        );
    }

    #[test]
    fn no_blueprint_available_resolves_to_none_without_error() {
        let definition = profile_less_definition();
        let catalog = BuildingNavigationBlueprintCatalog::from_definitions(Vec::new())
            .expect("empty catalog");
        assert!(
            resolve_building_navigation_blueprint(&definition, &catalog, None)
                .expect("absence is not an error")
                .is_none()
        );
    }

    #[test]
    fn disabled_generated_blueprint_reports_error() {
        let definition = profile_less_definition();
        let mut blueprint = two_story_hut_navigation_blueprint();
        blueprint.id = BuildingNavigationBlueprintId::new("hut_nav");
        blueprint.enabled = false;
        let catalog = catalog_with(blueprint);
        assert!(matches!(
            resolve_building_navigation_blueprint(&definition, &catalog, None),
            Err(BuildingNavigationBlueprintError::BlueprintDisabled(_))
        ));
    }

    #[test]
    fn missing_override_blueprint_id_reports_error() {
        let definition = profile_less_definition();
        let catalog = BuildingNavigationBlueprintCatalog::from_definitions(Vec::new())
            .expect("empty catalog");
        assert!(matches!(
            resolve_building_navigation_blueprint(
                &definition,
                &catalog,
                Some(&BuildingNavigationBlueprintInstanceOverride::catalog(
                    BuildingNavigationBlueprintId::new("absent")
                )),
            ),
            Err(BuildingNavigationBlueprintError::BlueprintMissing(_))
        ));
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
