//! Building asset variant creation from edited navigation blueprints (NV1.6).

use super::definition::BuildingDefinition;
use super::definition_id::BuildingDefinitionId;
use super::registry::BuildingCatalog;
use crate::data_import::{export_buildings_to_ron, DEV_BUILDING_CATALOG_RON_PATH};
use crate::world::building::category::BuildingCategoryCatalog;
use crate::world::building::interior::refresh_building_navigation_runtime;
use crate::world::building::navigation_blueprint::{
    BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision, BuildingNavigationBlueprintId,
    export_navigation_blueprint_catalog, prepare_blueprint_for_save,
};

/// Input for promoting an edited blueprint into a new building asset variant.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingVariantCreateInput {
    pub source_definition_id: BuildingDefinitionId,
    pub new_definition_id: BuildingDefinitionId,
    pub display_name: String,
    pub description: Option<String>,
    pub blueprint: BuildingNavigationBlueprint,
}

/// Outcome of creating a new building variant asset.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingVariantCreateOutcome {
    pub definition_id: BuildingDefinitionId,
    pub blueprint_id: BuildingNavigationBlueprintId,
    pub message: String,
}

/// Validate a candidate building definition id for variant creation.
pub fn validate_building_definition_id(
    id: &str,
    building_catalog: &BuildingCatalog,
) -> Result<(), String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("asset id must not be empty".into());
    }
    if trimmed != trimmed.to_lowercase() {
        return Err("asset id must be lowercase".into());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("asset id may only contain lowercase letters, digits, and underscores".into());
    }
    if building_catalog.get(&BuildingDefinitionId::new(trimmed)).is_some() {
        return Err(format!("asset id `{trimmed}` already exists"));
    }
    Ok(())
}

/// Suggest a unique-ish asset id from a display name and source id.
pub fn suggest_variant_definition_id(source_id: &str, display_name: &str) -> String {
    let slug: String = display_name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if slug.is_empty() {
        return format!("{source_id}_variant");
    }
    if slug == source_id {
        format!("{source_id}_variant")
    } else {
        slug
    }
}

/// Duplicate a building definition and fork its navigation blueprint into a new asset.
pub fn create_building_variant(
    building_catalog: &mut BuildingCatalog,
    category_catalog: &BuildingCategoryCatalog,
    nav_catalog: &mut BuildingNavigationBlueprintCatalog,
    nav_revision: &mut BuildingNavigationBlueprintCatalogRevision,
    input: BuildingVariantCreateInput,
) -> Result<BuildingVariantCreateOutcome, String> {
    let source = building_catalog
        .get(&input.source_definition_id)
        .ok_or_else(|| {
            format!(
                "source definition {} not found",
                input.source_definition_id.as_str()
            )
        })?
        .clone();

    validate_building_definition_id(input.new_definition_id.as_str(), building_catalog)?;
    if input.display_name.trim().is_empty() {
        return Err("variant display name must not be empty".into());
    }

    let blueprint_id = BuildingNavigationBlueprintId::new(format!(
        "{}_nav",
        input.new_definition_id.as_str()
    ));
    let mut blueprint = prepare_blueprint_for_save(input.blueprint)?;
    blueprint.id = blueprint_id.clone();
    blueprint.display_name = format!("{} Navigation", input.display_name.trim());
    if let Some(description) = input.description.as_deref().map(str::trim).filter(|s| !s.is_empty())
    {
        blueprint
            .metadata
            .extensions
            .insert("description".to_string(), description.to_string());
    }
    blueprint.metadata.extensions.insert(
        "variant_of".to_string(),
        input.source_definition_id.as_str().to_string(),
    );
    blueprint.metadata.extensions.insert(
        "created_by".to_string(),
        "dev_variant".to_string(),
    );

    nav_catalog
        .upsert(blueprint)
        .map_err(|err| err.to_string())?;
    export_navigation_blueprint_catalog(nav_catalog)?;
    nav_revision.0 = nav_revision.0.saturating_add(1);

    let mut variant = source;
    variant.id = input.new_definition_id.clone();
    variant.display_name = input.display_name.trim().to_string();
    variant.navigation_blueprint_id = Some(blueprint_id.as_str().to_string());

    building_catalog
        .upsert(variant, category_catalog)
        .map_err(|err| err.to_string())?;
    export_building_catalog_snapshot(building_catalog, category_catalog)?;

    Ok(BuildingVariantCreateOutcome {
        definition_id: input.new_definition_id.clone(),
        blueprint_id: blueprint_id.clone(),
        message: format!(
            "Created building variant `{}` with navigation blueprint `{}`",
            input.new_definition_id.as_str(),
            blueprint_id.as_str()
        ),
    })
}

/// Export the current in-memory building catalog to the dev RON snapshot.
pub fn export_building_catalog_snapshot(
    building_catalog: &BuildingCatalog,
    category_catalog: &BuildingCategoryCatalog,
) -> Result<(), String> {
    export_buildings_to_ron(
        std::path::Path::new(DEV_BUILDING_CATALOG_RON_PATH),
        category_catalog.definitions(),
        building_catalog.definitions(),
    )
    .map_err(|err| err.to_string())
}

/// Replace a placed building instance to use a different definition id.
pub fn replace_building_instance_definition(
    world: &mut crate::world::WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &crate::world::InteriorProfileCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: crate::world::BuildingId,
    new_definition_id: BuildingDefinitionId,
) -> Result<(), String> {
    if building_catalog.get(&new_definition_id).is_none() {
        return Err(format!(
            "definition {} does not exist",
            new_definition_id.as_str()
        ));
    }
    let record = world
        .get_building(building_id)
        .ok_or_else(|| format!("building #{} not found", building_id.raw()))?;
    let activated = record.interior.activated;
    world.mutate_building(building_id, |building| {
        building.definition_id = new_definition_id;
        building.interior.navigation_blueprint_override = None;
    });
    if activated {
        refresh_building_navigation_runtime(
            world,
            building_catalog,
            interior_catalog,
            nav_catalog,
            building_id,
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::two_story_hut_navigation_blueprint;
    use crate::world::BuildingCategoryCatalog;

    #[test]
    fn suggest_variant_id_from_display_name() {
        assert_eq!(
            suggest_variant_definition_id("barn", "Barn Large"),
            "barn_large"
        );
    }

    #[test]
    fn duplicate_id_rejected() {
        let catalog = BuildingCatalog::default();
        assert!(validate_building_definition_id("hut", &catalog).is_err());
        assert!(validate_building_definition_id("barn_large", &catalog).is_ok());
    }

    #[test]
    fn create_variant_forks_definition_and_blueprint() {
        let categories = BuildingCategoryCatalog::default();
        let mut building_catalog = BuildingCatalog::default();
        let mut nav_catalog = crate::world::BuildingNavigationBlueprintCatalog::default();
        let mut nav_revision = crate::world::BuildingNavigationBlueprintCatalogRevision::default();

        let source_id = BuildingDefinitionId::new("hut");
        let blueprint = two_story_hut_navigation_blueprint();
        let outcome = create_building_variant(
            &mut building_catalog,
            &categories,
            &mut nav_catalog,
            &mut nav_revision,
            BuildingVariantCreateInput {
                source_definition_id: source_id.clone(),
                new_definition_id: BuildingDefinitionId::new("hut_open_front"),
                display_name: "Hut Open Front".into(),
                description: Some("Test variant".into()),
                blueprint,
            },
        )
        .expect("variant");

        assert_eq!(outcome.definition_id.as_str(), "hut_open_front");
        assert_eq!(outcome.blueprint_id.as_str(), "hut_open_front_nav");
        let variant = building_catalog
            .get(&outcome.definition_id)
            .expect("variant definition");
        assert_eq!(variant.display_name, "Hut Open Front");
        assert_eq!(
            variant.navigation_blueprint_id.as_deref(),
            Some("hut_open_front_nav")
        );
        assert!(building_catalog.get(&source_id).is_some());
        assert!(nav_catalog.get(&outcome.blueprint_id).is_some());
        assert!(nav_catalog.get(&crate::world::BuildingNavigationBlueprintId::new("two_story_hut")).is_some());
    }
}
