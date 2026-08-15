//! Blueprint persistence actions for dev editor and runtime refresh (NV1.5).

use super::catalog::{
    BuildingNavigationBlueprintCatalog, BuildingNavigationBlueprintCatalogRevision,
};
use super::definition::{BuildingNavigationBlueprint, BuildingNavigationBlueprintInstanceOverride};
use super::edit::prepare_blueprint_for_save;
use super::pipeline::export_navigation_blueprint_catalog;
use super::resolve::resolve_building_navigation_blueprint;
use super::source::{BlueprintAuthoritySource, classify_blueprint_authority};
use crate::world::building::catalog::{BuildingCatalog, BuildingDefinitionId};
use crate::world::building::interior::{
    InteriorProfileCatalog, InteriorProfileId, NavigationReconcileOutcome,
    reconcile_building_navigation_runtime,
};
use crate::world::{BuildingId, DoodadCatalog, FootprintCatalog, OccupancyCatalogs, WorldData};

use super::id::blueprint_id_for_building;

/// Catalogs needed to activate an interior that is not yet active.
///
/// Saving a blueprint must be able to activate a building that had nothing to
/// activate before, not only refresh already-active ones (IN-11b).
#[derive(Clone, Copy)]
pub struct InteriorActivationCatalogs<'a> {
    pub interior: &'a InteriorProfileCatalog,
    pub doodad: &'a DoodadCatalog,
    pub footprint: &'a FootprintCatalog,
}

/// Per-instance results of propagating a blueprint change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlueprintPropagationCounts {
    pub affected: usize,
    pub activated: usize,
    pub refreshed: usize,
    pub skipped: usize,
    pub failed: usize,
}

impl BlueprintPropagationCounts {
    pub fn summary(&self) -> String {
        format!(
            "activated {}, refreshed {}, skipped {}, failed {}",
            self.activated, self.refreshed, self.skipped, self.failed
        )
    }
}

/// Outcome of a blueprint persistence action.
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintPersistenceOutcome {
    pub message: String,
    pub authority: BlueprintAuthoritySource,
    pub propagation: BlueprintPropagationCounts,
}

/// Count loaded building instances that would inherit an asset-default blueprint change.
pub fn count_inheriting_instances(
    world: &WorldData,
    definition_id: &BuildingDefinitionId,
) -> usize {
    world
        .sorted_building_ids()
        .into_iter()
        .filter(|building_id| {
            world.get_building(*building_id).is_some_and(|record| {
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
    activation: InteriorActivationCatalogs<'_>,
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
    world.mutate_building(building_id, |building| {
        building.interior.navigation_blueprint_override = Some(
            BuildingNavigationBlueprintInstanceOverride::inline(prepared),
        );
    });

    let mut counts = BlueprintPropagationCounts {
        affected: 1,
        ..Default::default()
    };
    propagate_to_instance(
        world,
        building_catalog,
        activation,
        nav_catalog,
        building_id,
        &mut counts,
    );

    Ok(BlueprintPersistenceOutcome {
        message: format!(
            "Saved instance blueprint override for building #{}: {}",
            building_id.raw(),
            counts.summary()
        ),
        authority: classify_blueprint_authority(
            definition,
            nav_catalog,
            world
                .get_building(building_id)
                .and_then(|record| record.interior.navigation_blueprint_override.as_ref()),
        ),
        propagation: counts,
    })
}

/// Persist the edited blueprint as the asset default in the navigation catalog.
pub fn apply_blueprint_to_asset(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    activation: InteriorActivationCatalogs<'_>,
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
    let mut counts = BlueprintPropagationCounts::default();
    for building_id in world.sorted_building_ids() {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if record.definition_id != *definition_id {
            continue;
        }
        if record.interior.navigation_blueprint_override.is_some() {
            // An instance override outranks the asset default.
            counts.skipped = counts.skipped.saturating_add(1);
            continue;
        }
        counts.affected = counts.affected.saturating_add(1);
        propagate_to_instance(
            world,
            building_catalog,
            activation,
            nav_catalog,
            building_id,
            &mut counts,
        );
    }

    Ok(BlueprintPersistenceOutcome {
        message: format!(
            "Applied blueprint {} to asset default ({} inheriting instance(s)): {}",
            canonical_id.as_str(),
            inheriting,
            counts.summary()
        ),
        authority: BlueprintAuthoritySource::AssetDefault,
        propagation: counts,
    })
}

/// Rebuild or newly create runtime navigation for one instance after a blueprint change.
///
/// A building that was never activated must be able to activate now: it may have had
/// no resolvable blueprint (or no interior profile) at completion time.
fn propagate_to_instance(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    activation: InteriorActivationCatalogs<'_>,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    counts: &mut BlueprintPropagationCounts,
) {
    let occupancy = OccupancyCatalogs {
        doodad: activation.doodad,
        building: building_catalog,
        footprint: activation.footprint,
    };
    match reconcile_building_navigation_runtime(
        world,
        building_catalog,
        activation.interior,
        activation.doodad,
        occupancy,
        nav_catalog,
        building_id,
        true,
    ) {
        Ok(NavigationReconcileOutcome::Activated(_)) => {
            counts.activated = counts.activated.saturating_add(1);
        }
        Ok(NavigationReconcileOutcome::Refreshed) => {
            counts.refreshed = counts.refreshed.saturating_add(1);
        }
        Ok(NavigationReconcileOutcome::NotNeeded) => {
            counts.skipped = counts.skipped.saturating_add(1);
        }
        Err(_) => counts.failed = counts.failed.saturating_add(1),
    }
}

/// Remove the selected building's instance override and resolve back to asset/generated.
pub fn reset_instance_to_asset(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    activation: InteriorActivationCatalogs<'_>,
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

    world.mutate_building(building_id, |building| {
        building.interior.navigation_blueprint_override = None;
    });

    let authority = classify_blueprint_authority(definition, nav_catalog, None);
    let resolved = resolve_building_navigation_blueprint(definition, nav_catalog, None)
        .map_err(|err| err.to_string())?;
    if resolved.is_none() {
        return Err("no asset default or generated blueprint available after reset".into());
    }

    let mut counts = BlueprintPropagationCounts {
        affected: 1,
        ..Default::default()
    };
    propagate_to_instance(
        world,
        building_catalog,
        activation,
        nav_catalog,
        building_id,
        &mut counts,
    );

    Ok(BlueprintPersistenceOutcome {
        message: format!(
            "Reset building #{} to {} blueprint: {}",
            building_id.raw(),
            authority.label(),
            counts.summary()
        ),
        authority,
        propagation: counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::fixtures::one_region_doorless_navigation_blueprint;
    use crate::world::{
        Affiliation, BuildingCategoryCatalog, BuildingLifecycleState, BuildingOwnership,
        ChunkCoord, ChunkLayout, InteriorActivationStatus, place_player_building,
        set_building_lifecycle_stage, starter_building_definitions,
    };
    use bevy::prelude::*;

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            crate::world::ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    /// Hut definition without an interior profile, matching the real import.
    fn profile_less_catalog() -> BuildingCatalog {
        let definitions = starter_building_definitions()
            .into_iter()
            .map(|mut definition| {
                if definition.id == BuildingDefinitionId::new("hut") {
                    definition.interior_profile_id = None;
                    definition.navigation_blueprint_id = None;
                }
                definition
            })
            .collect();
        BuildingCatalog::from_definitions(definitions, &BuildingCategoryCatalog::default())
            .expect("catalog")
    }

    /// IN-11b: saving a blueprint for a building that was never activated must
    /// activate it, not silently report "refreshed 0".
    #[test]
    fn saving_instance_blueprint_activates_previously_unactivated_building() {
        let building_catalog = profile_less_catalog();
        let doodad_catalog = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let interior = InteriorProfileCatalog::default();
        let nav_catalog = BuildingNavigationBlueprintCatalog::from_definitions(Vec::new())
            .expect("empty nav catalog");
        let occupancy = OccupancyCatalogs {
            building: &building_catalog,
            doodad: &doodad_catalog,
            footprint: &footprint,
        };

        let mut world = flat_world();
        let building_id = place_player_building(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                crate::world::LocalPosition::new(Vec3::new(80.0, 0.0, 80.0)),
            ),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            occupancy,
        )
        .expect("place")
        .id;
        set_building_lifecycle_stage(
            &mut world,
            &building_catalog,
            &interior,
            &doodad_catalog,
            occupancy,
            Some(&nav_catalog),
            building_id,
            BuildingLifecycleState::Complete,
            1.0,
        )
        .expect("complete");
        assert!(
            !world.get_building(building_id).unwrap().interior.activated,
            "precondition: nothing was activatable yet"
        );

        let outcome = save_instance_blueprint(
            &mut world,
            &building_catalog,
            InteriorActivationCatalogs {
                interior: &interior,
                doodad: &doodad_catalog,
                footprint: &footprint,
            },
            &nav_catalog,
            building_id,
            one_region_doorless_navigation_blueprint(),
        )
        .expect("save");

        assert_eq!(
            outcome.propagation,
            BlueprintPropagationCounts {
                affected: 1,
                activated: 1,
                refreshed: 0,
                skipped: 0,
                failed: 0,
            },
            "{}",
            outcome.message
        );
        assert_eq!(
            world
                .interior_activation_outcomes()
                .get(building_id)
                .map(|outcome| outcome.status.clone()),
            Some(InteriorActivationStatus::NavigationWithoutProfile)
        );
        assert!(
            world
                .building_navigation_runtime()
                .get(building_id)
                .is_some()
        );
    }

    /// An already-active building is refreshed rather than reactivated.
    #[test]
    fn saving_instance_blueprint_refreshes_active_building() {
        let building_catalog = profile_less_catalog();
        let doodad_catalog = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let interior = InteriorProfileCatalog::default();
        let nav_catalog = BuildingNavigationBlueprintCatalog::from_definitions(Vec::new())
            .expect("empty nav catalog");
        let occupancy = OccupancyCatalogs {
            building: &building_catalog,
            doodad: &doodad_catalog,
            footprint: &footprint,
        };

        let mut world = flat_world();
        let building_id = place_player_building(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                crate::world::LocalPosition::new(Vec3::new(80.0, 0.0, 80.0)),
            ),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            occupancy,
        )
        .expect("place")
        .id;
        world
            .mutate_building(building_id, |record| {
                record.interior.navigation_blueprint_override =
                    Some(BuildingNavigationBlueprintInstanceOverride::inline(
                        one_region_doorless_navigation_blueprint(),
                    ));
            })
            .expect("building");
        set_building_lifecycle_stage(
            &mut world,
            &building_catalog,
            &interior,
            &doodad_catalog,
            occupancy,
            Some(&nav_catalog),
            building_id,
            BuildingLifecycleState::Complete,
            1.0,
        )
        .expect("complete");
        assert!(world.get_building(building_id).unwrap().interior.activated);

        let outcome = save_instance_blueprint(
            &mut world,
            &building_catalog,
            InteriorActivationCatalogs {
                interior: &interior,
                doodad: &doodad_catalog,
                footprint: &footprint,
            },
            &nav_catalog,
            building_id,
            one_region_doorless_navigation_blueprint(),
        )
        .expect("save");

        assert_eq!(outcome.propagation.refreshed, 1, "{}", outcome.message);
        assert_eq!(outcome.propagation.activated, 0, "{}", outcome.message);
    }
}

/// Save target must be the same id the runtime resolver reads, in every build config.
fn canonical_asset_blueprint_id(
    definition: &crate::world::building::catalog::BuildingDefinition,
) -> super::id::BuildingNavigationBlueprintId {
    blueprint_id_for_building(definition)
}
