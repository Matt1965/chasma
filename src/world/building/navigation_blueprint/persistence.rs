//! Blueprint persistence actions for dev editor and runtime refresh (NV1.5).

use super::catalog::{
    BuildingNavigationBlueprintCatalog, BuildingNavigationBlueprintCatalogRevision,
};
use super::definition::{
    BuildingNavigationBlueprint, BuildingNavigationBlueprintInstanceOverride,
};
use super::edit::prepare_blueprint_for_save;
use super::pipeline::export_navigation_blueprint_catalog;
use super::resolve::resolve_building_navigation_blueprint;
use super::source::{BlueprintAuthoritySource, classify_blueprint_authority};
use crate::world::building::catalog::{BuildingCatalog, BuildingDefinitionId};
use crate::world::building::interior::{
    InteriorProfileCatalog, InteriorProfileId, refresh_building_navigation_runtime,
};
use crate::world::{BuildingId, WorldData};

#[cfg(feature = "data-import")]
use super::generate::blueprint_id_for_building;

/// Outcome of a blueprint persistence action.
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintPersistenceOutcome {
    pub message: String,
    pub authority: BlueprintAuthoritySource,
}

/// Count loaded building instances that would inherit an asset-default blueprint change.
pub fn count_inheriting_instances(world: &WorldData, definition_id: &BuildingDefinitionId) -> usize {
    world
        .sorted_building_ids()
        .into_iter()
        .filter(|building_id| {
            world
                .get_building(*building_id)
                .is_some_and(|record| {
                    record.definition_id == *definition_id
                        && record.interior.navigation_blueprint_override.is_none()
                })
        })
        .count()
}

/// Persist the edited blueprint as an inline instance override.
pub fn save_instance_blueprint(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    blueprint: BuildingNavigationBlueprint,
) -> Result<BlueprintPersistenceOutcome, String> {
    let record = world
        .get_building(building_id)
        .ok_or_else(|| format!("building #{} not found", building_id.raw()))?;
    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| format!("definition {} missing", record.definition_id.as_str()))?;

    let prepared = prepare_blueprint_for_save(blueprint)?;
    let activated = record.interior.activated;
    world.mutate_building(building_id, |building| {
        building.interior.navigation_blueprint_override =
            Some(BuildingNavigationBlueprintInstanceOverride::inline(prepared));
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

    Ok(BlueprintPersistenceOutcome {
        message: format!(
            "Saved instance blueprint override for building #{}",
            building_id.raw()
        ),
        authority: classify_blueprint_authority(
            definition,
            nav_catalog,
            world
                .get_building(building_id)
                .and_then(|record| record.interior.navigation_blueprint_override.as_ref()),
        ),
    })
}

/// Persist the edited blueprint as the asset default in the navigation catalog.
pub fn apply_blueprint_to_asset(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    nav_catalog: &mut BuildingNavigationBlueprintCatalog,
    nav_revision: &mut BuildingNavigationBlueprintCatalogRevision,
    definition_id: &BuildingDefinitionId,
    blueprint: BuildingNavigationBlueprint,
) -> Result<BlueprintPersistenceOutcome, String> {
    let definition = building_catalog
        .get(definition_id)
        .ok_or_else(|| format!("definition {} missing", definition_id.as_str()))?;

    let mut prepared = prepare_blueprint_for_save(blueprint)?;
    let canonical_id = canonical_asset_blueprint_id(definition);
    prepared.id = canonical_id.clone();

    nav_catalog
        .upsert(prepared)
        .map_err(|err| err.to_string())?;
    export_navigation_blueprint_catalog(nav_catalog)?;
    nav_revision.0 = nav_revision.0.saturating_add(1);

    let inheriting = count_inheriting_instances(world, definition_id);
    let mut refreshed = 0usize;
    for building_id in world.sorted_building_ids() {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if record.definition_id != *definition_id {
            continue;
        }
        if record.interior.navigation_blueprint_override.is_some() {
            continue;
        }
        if !record.interior.activated {
            continue;
        }
        if refresh_building_navigation_runtime(
            world,
            building_catalog,
            interior_catalog,
            nav_catalog,
            building_id,
        )
        .is_ok()
        {
            refreshed = refreshed.saturating_add(1);
        }
    }

    Ok(BlueprintPersistenceOutcome {
        message: format!(
            "Applied blueprint {} to asset default ({} inheriting instance(s), refreshed {})",
            canonical_id.as_str(),
            inheriting,
            refreshed
        ),
        authority: BlueprintAuthoritySource::AssetDefault,
    })
}

/// Remove the selected building's instance override and resolve back to asset/generated.
pub fn reset_instance_to_asset(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
) -> Result<BlueprintPersistenceOutcome, String> {
    let record = world
        .get_building(building_id)
        .ok_or_else(|| format!("building #{} not found", building_id.raw()))?;
    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| format!("definition {} missing", record.definition_id.as_str()))?;

    if record.interior.navigation_blueprint_override.is_none() {
        return Err("building has no instance blueprint override to reset".into());
    }

    let activated = record.interior.activated;
    world.mutate_building(building_id, |building| {
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

    let authority = classify_blueprint_authority(definition, nav_catalog, None);
    let resolved = resolve_building_navigation_blueprint(definition, nav_catalog, None)
        .map_err(|err| err.to_string())?;
    if resolved.is_none() {
        return Err("no asset default or generated blueprint available after reset".into());
    }

    Ok(BlueprintPersistenceOutcome {
        message: format!(
            "Reset building #{} to {} blueprint",
            building_id.raw(),
            authority.label()
        ),
        authority,
    })
}

fn canonical_asset_blueprint_id(
    definition: &crate::world::building::catalog::BuildingDefinition,
) -> super::id::BuildingNavigationBlueprintId {
    #[cfg(feature = "data-import")]
    {
        return blueprint_id_for_building(definition);
    }
    #[cfg(not(feature = "data-import"))]
    {
        use super::id::BuildingNavigationBlueprintId;
        if let Some(id) = &definition.navigation_blueprint_id {
            BuildingNavigationBlueprintId::new(id.clone())
        } else {
            BuildingNavigationBlueprintId::new(format!("{}_nav", definition.id.as_str()))
        }
    }
}
