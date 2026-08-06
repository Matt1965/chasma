use bevy::prelude::*;
use std::collections::HashSet;

use super::catalog::{InteriorChildKind, InteriorProfile};
use super::door::DoorRecord;
use super::door_store::DoorStore;
use super::error::InteriorError;
use super::id::InteriorProfileId;
use super::outcome::{InteriorActivationOutcome, InteriorActivationStatus};
use crate::world::building::catalog::{BuildingCatalog, BuildingDefinition};
use crate::world::building::navigation_blueprint::{
    BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
    ResolvedBuildingNavigationBlueprint, blueprint_portal_templates, blueprint_space_templates,
    build_navigation_runtime, classify_blueprint_authority, register_building_navigation_profile,
    resolve_building_navigation_blueprint,
};
use crate::world::building::record::BuildingRecord;
use crate::world::building::state::BuildingInteriorState;
use crate::world::building::state::BuildingLifecycleState;
use crate::world::{
    BuildingId, BuildingSource, DoodadCatalog, DoodadPlacementOverrides, DoodadSource,
    OccupancyCatalogs, PortalId, PortalRecord, SpaceId, WorldData, WorldPosition,
    building_model_world_transform, create_building, create_doodad,
    register_building_space_profile,
};

/// Activate interior navigation and, when present, interior presentation.
///
/// Navigation is owned by the resolved navigation blueprint; the interior profile is
/// optional presentation (children, authored doors). Either may activate without the
/// other, so a building with a valid blueprint and no profile still gets runtime
/// spaces and portals (IN-11b).
pub fn activate_building_interior(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &super::catalog::InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    nav_catalog: Option<&BuildingNavigationBlueprintCatalog>,
    building_id: BuildingId,
    profile_id: Option<&InteriorProfileId>,
) -> Result<InteriorActivationOutcome, InteriorError> {
    let record = world
        .get_building(building_id)
        .cloned()
        .ok_or(InteriorError::ParentBuildingMissing(building_id))?;
    let definition = building_catalog.get(&record.definition_id).ok_or_else(|| {
        InteriorError::InteriorSpawnFailed {
            building_id,
            reason: format!("missing definition `{}`", record.definition_id.as_str()),
        }
    })?;

    if record.interior.activated && !world.door_store().building_door_ids(building_id).is_empty() {
        record_outcome(
            world,
            InteriorActivationOutcome::new(
                building_id,
                record.definition_id.clone(),
                InteriorActivationStatus::AlreadyActivated,
            ),
        );
        return Err(InteriorError::BuildingInteriorAlreadyActive(building_id));
    }

    // Navigation authority, resolved independently of the interior profile.
    let blueprint =
        resolve_navigation_for_activation(&record, definition, nav_catalog, building_id).map_err(
            |(err, reason)| {
                record_outcome(
                    world,
                    InteriorActivationOutcome::new(
                        building_id,
                        record.definition_id.clone(),
                        InteriorActivationStatus::BlueprintResolutionFailed { reason },
                    ),
                );
                err
            },
        )?;

    // Presentation authority, resolved independently of the blueprint.
    let profile = profile_id.and_then(|id| interior_catalog.get(id));
    if profile.is_none() && blueprint.is_none() {
        // The profile was the only declared authority and it does not resolve, so
        // nothing at all can be activated.
        if let Some(id) = profile_id {
            record_outcome(
                world,
                InteriorActivationOutcome::new(
                    building_id,
                    record.definition_id.clone(),
                    InteriorActivationStatus::NavigationProfileMissing {
                        profile_key: id.as_str().to_string(),
                    },
                ),
            );
            return Err(InteriorError::MissingInteriorProfile(id.clone()));
        }
        let outcome = InteriorActivationOutcome::new(
            building_id,
            record.definition_id.clone(),
            InteriorActivationStatus::NoBlueprintNoProfile,
        );
        record_outcome(world, outcome.clone());
        return Ok(outcome);
    }

    let layout = world.layout();
    let (space_keys, portal_keys) = if let Some(resolved) = blueprint.as_ref() {
        let blueprint = resolved.blueprint();
        let spaces = blueprint_space_templates(blueprint);
        let portals = blueprint_portal_templates(blueprint);
        let keys = register_building_navigation_profile(
            world.space_registry_mut(),
            &record,
            definition,
            layout,
            &spaces,
            &portals,
        );
        let model = building_model_world_transform(definition, &record.placement, layout);
        world
            .building_navigation_runtime_mut()
            .insert(build_navigation_runtime(
                building_id,
                blueprint,
                model,
                &keys.0,
                &keys.1,
            ));
        let (space_keys, mut portal_keys) = keys;
        if let Some(profile) = profile {
            supplement_door_portals_from_profile(
                world.space_registry_mut(),
                &record,
                definition,
                layout,
                profile,
                &space_keys,
                &mut portal_keys,
            )?;
        }
        (space_keys, portal_keys)
    } else {
        let profile = profile.expect("profile present when no blueprint resolved");
        register_building_space_profile(
            world.space_registry_mut(),
            &record,
            layout,
            &profile.spaces,
            &profile.portals,
        )
    };

    let mut door_ids = Vec::new();
    let mut child_doodad_ids = record
        .interior
        .child_doodad_ids
        .iter()
        .map(|id| crate::world::DoodadId::new(*id))
        .collect::<Vec<_>>();
    let mut child_building_ids = record
        .interior
        .child_building_ids
        .iter()
        .map(|id| BuildingId::new(*id))
        .collect::<Vec<_>>();

    if let Some(profile) = profile {
        let mut seen_door_portal_keys = HashSet::new();
        for template in &profile.doors {
            if !seen_door_portal_keys.insert(template.portal_key) {
                return Err(InteriorError::InvalidDoorPortal {
                    door_key: template.key.to_string(),
                    portal_key: template.portal_key.to_string(),
                });
            }
            if !profile_door_controls_portal(
                blueprint.as_ref().map(|resolved| resolved.blueprint()),
                profile,
                template.key,
                template.portal_key,
            ) {
                continue;
            }
            let Some(portal_id) = portal_keys.get(template.portal_key).copied() else {
                continue;
            };
            let door_id = world.door_store_mut().allocate_door_id();
            world.door_store_mut().insert_door(DoorRecord {
                id: door_id,
                owning_building_id: building_id,
                portal_id,
                definition_key: template.key.to_string(),
                state: template.initial_state,
                access: template.access,
            })?;
            DoorStore::sync_portal_enabled(world, door_id)?;
            door_ids.push(door_id);
        }

        let skip_children = !record.interior.child_doodad_ids.is_empty()
            || !record.interior.child_building_ids.is_empty();
        if !skip_children {
            child_doodad_ids.clear();
            child_building_ids.clear();
            spawn_interior_children(
                world,
                building_catalog,
                doodad_catalog,
                occupancy,
                &record,
                profile,
                &space_keys,
                &mut child_doodad_ids,
                &mut child_building_ids,
            )?;
        }
    }

    let space_ids: Vec<String> = world
        .space_registry()
        .building_space_ids(building_id)
        .iter()
        .map(|space_id| space_id.raw().to_string())
        .collect();

    let preserved_override = record.interior.navigation_blueprint_override.clone();
    let recorded_profile_id = profile_id
        .map(|id| id.as_str().to_string())
        .or(record.interior.profile_id.clone());

    world.mutate_building(building_id, |building| {
        building.spaces.space_ids = space_ids;
        building.interior = BuildingInteriorState {
            profile_id: recorded_profile_id,
            navigation_blueprint_override: preserved_override,
            door_ids: door_ids.iter().map(|id| id.raw()).collect(),
            child_doodad_ids: child_doodad_ids.iter().map(|id| id.raw()).collect(),
            child_building_ids: child_building_ids.iter().map(|id| id.raw()).collect(),
            activated: true,
            interior_space_id: None,
        };
    });

    let status = match (blueprint.is_some(), profile.is_some(), profile_id) {
        (true, true, _) => InteriorActivationStatus::NavigationAndProfile,
        (true, false, Some(id)) => InteriorActivationStatus::NavigationProfileMissing {
            profile_key: id.as_str().to_string(),
        },
        (true, false, None) => InteriorActivationStatus::NavigationWithoutProfile,
        (false, _, _) => InteriorActivationStatus::ProfileWithoutNavigation,
    };
    let mut outcome =
        InteriorActivationOutcome::new(building_id, record.definition_id.clone(), status);
    outcome.profile_id = profile.map(|profile| profile.id.clone());
    outcome.blueprint_id = blueprint
        .as_ref()
        .map(|resolved| resolved.blueprint().id.clone());
    if let Some(catalog) = nav_catalog {
        outcome.blueprint_authority = classify_blueprint_authority(
            definition,
            catalog,
            record.interior.navigation_blueprint_override.as_ref(),
        );
    }
    populate_runtime_counts(world, building_id, &mut outcome);
    record_outcome(world, outcome.clone());
    Ok(outcome)
}

/// Resolve navigation blueprint authority. Errors carry a diagnostic reason.
fn resolve_navigation_for_activation<'a>(
    record: &BuildingRecord,
    definition: &BuildingDefinition,
    nav_catalog: Option<&'a BuildingNavigationBlueprintCatalog>,
    building_id: BuildingId,
) -> Result<Option<ResolvedBuildingNavigationBlueprint<'a>>, (InteriorError, String)> {
    match nav_catalog {
        Some(catalog) => {
            let resolved = resolve_building_navigation_blueprint(
                definition,
                catalog,
                record.interior.navigation_blueprint_override.as_ref(),
            );
            if record.interior.navigation_blueprint_override.is_some() {
                resolved.map_err(|err| {
                    let reason = err.to_string();
                    (
                        InteriorError::InteriorSpawnFailed {
                            building_id,
                            reason: reason.clone(),
                        },
                        reason,
                    )
                })
            } else {
                Ok(resolved.ok().flatten())
            }
        }
        None if record.interior.navigation_blueprint_override.is_some() => {
            let reason = "navigation catalog required for blueprint override".to_string();
            Err((
                InteriorError::InteriorSpawnFailed {
                    building_id,
                    reason: reason.clone(),
                },
                reason,
            ))
        }
        None => Ok(None),
    }
}

/// Whether a profile door owns the passage registered under `portal_key`.
///
/// A blueprint entrance is doorless unless it names a door explicitly or the profile
/// itself authors a portal template of the same key. Without this, any profile whose
/// door key merely collided with an entrance key would silently close that entrance.
/// The criterion is the authored key relationship, never the blueprint authority layer.
fn profile_door_controls_portal(
    blueprint: Option<&BuildingNavigationBlueprint>,
    profile: &InteriorProfile,
    door_key: &str,
    portal_key: &str,
) -> bool {
    let Some(blueprint) = blueprint else {
        // Profile-only interior: profile doors own the profile's own portals.
        return true;
    };
    if let Some(entrance) = blueprint
        .entrances
        .iter()
        .find(|entrance| entrance.key == portal_key)
    {
        return match entrance.door_key.as_deref() {
            Some(key) => key == door_key,
            None => profile
                .portals
                .iter()
                .any(|portal| portal.key == portal_key),
        };
    }
    if let Some(connection) = blueprint
        .region_connections
        .iter()
        .find(|connection| connection.key == portal_key)
    {
        return match connection.door_key.as_deref() {
            Some(key) => key == door_key,
            None => profile
                .portals
                .iter()
                .any(|portal| portal.key == portal_key),
        };
    }
    true
}

fn populate_runtime_counts(
    world: &WorldData,
    building_id: BuildingId,
    outcome: &mut InteriorActivationOutcome,
) {
    if let Some(runtime) = world.building_navigation_runtime().get(building_id) {
        outcome.runtime_floor_count = runtime.floors.len();
        outcome.runtime_region_count = runtime.regions.len();
    }
    // Activation is a rare event, so an owner scan here mirrors
    // `SpaceRegistry::remove_building` rather than adding a new index.
    outcome.runtime_portal_count = world
        .space_registry()
        .portals()
        .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
        .count();
}

fn record_outcome(world: &mut WorldData, outcome: InteriorActivationOutcome) {
    world.interior_activation_outcomes_mut().record(outcome);
}

/// Activate interior data when a building is already [`BuildingLifecycleState::Complete`].
///
/// Used by dev spawn and other instant-complete authoring paths that skip construction.
pub fn try_activate_interior_if_complete(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &super::catalog::InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    nav_catalog: Option<&BuildingNavigationBlueprintCatalog>,
    building_id: BuildingId,
) -> Result<(), InteriorError> {
    let record = world
        .get_building(building_id)
        .ok_or(InteriorError::ParentBuildingMissing(building_id))?;
    if record.lifecycle_state != BuildingLifecycleState::Complete {
        return Ok(());
    }
    if record.interior.activated {
        return Ok(());
    }
    let definition = building_catalog.get(&record.definition_id).ok_or_else(|| {
        InteriorError::InteriorSpawnFailed {
            building_id,
            reason: format!("missing definition `{}`", record.definition_id.as_str()),
        }
    })?;
    activate_interior_for_definition(
        world,
        building_catalog,
        interior_catalog,
        doodad_catalog,
        occupancy,
        nav_catalog,
        building_id,
        definition,
    )
    .map(|_| ())
}

fn activate_interior_for_definition(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &super::catalog::InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    nav_catalog: Option<&BuildingNavigationBlueprintCatalog>,
    building_id: BuildingId,
    definition: &BuildingDefinition,
) -> Result<InteriorActivationOutcome, InteriorError> {
    // No profile gate: the navigation blueprint is an independent authority and may
    // activate on its own. `activate_building_interior` records why it skipped.
    let profile_id = definition
        .interior_profile_id
        .as_deref()
        .map(InteriorProfileId::new);
    activate_building_interior(
        world,
        building_catalog,
        interior_catalog,
        doodad_catalog,
        occupancy,
        nav_catalog,
        building_id,
        profile_id.as_ref(),
    )
}

/// Resolve an interior-profile space key against blueprint-registered space keys.
fn resolve_profile_space_key(
    space_keys: &std::collections::BTreeMap<String, SpaceId>,
    key: &str,
) -> Option<SpaceId> {
    if let Some(id) = space_keys.get(key) {
        return Some(*id);
    }
    let qualified = format!("{key}/main");
    space_keys.get(&qualified).copied()
}
/// Register door-linked portals from the interior profile when the blueprint omits them.
fn supplement_door_portals_from_profile(
    registry: &mut crate::world::SpaceRegistry,
    building: &BuildingRecord,
    definition: &BuildingDefinition,
    layout: crate::world::ChunkLayout,
    profile: &InteriorProfile,
    space_keys: &std::collections::BTreeMap<String, SpaceId>,
    portal_keys: &mut std::collections::BTreeMap<String, PortalId>,
) -> Result<(), InteriorError> {
    let model = building_model_world_transform(definition, &building.placement, layout);
    let floor_y_for = |space_key: &str| {
        profile
            .spaces
            .iter()
            .find(|space| space.key == space_key)
            .map(|space| space.local_floor_y)
            .unwrap_or(0.0)
    };

    for door in &profile.doors {
        if portal_keys.contains_key(door.portal_key) {
            continue;
        }
        let template = profile
            .portals
            .iter()
            .find(|portal| portal.key == door.portal_key)
            .ok_or_else(|| InteriorError::InvalidDoorPortal {
                door_key: door.key.to_string(),
                portal_key: door.portal_key.to_string(),
            })?;
        let Some(from_space) = resolve_profile_space_key(space_keys, template.from_space_key)
        else {
            continue;
        };
        let Some(to_space) = resolve_profile_space_key(space_keys, template.to_space_key) else {
            continue;
        };
        let from_floor_y = floor_y_for(template.from_space_key);
        let from_local = Vec3::new(
            template.from_local_xz.x,
            from_floor_y,
            template.from_local_xz.y,
        );
        let from_global = model.transform_point(from_local);
        let to_global = model.transform_point(template.to_local_position);
        let portal_id = registry.allocate_portal_id();
        registry.insert_portal(PortalRecord {
            id: portal_id,
            portal_type: template.portal_type,
            from_space,
            to_space,
            from_center_global_xz: Vec2::new(from_global.x, from_global.z),
            from_radius_meters: template.from_radius_meters,
            to_position: WorldPosition::from_global(to_global, layout),
            traversal_cost: 1.0,
            bidirectional: template.bidirectional,
            enabled: true,
            owning_building_id: Some(building.id),
        });
        portal_keys.insert(template.key.to_string(), portal_id);
    }
    Ok(())
}

fn spawn_interior_children(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    parent: &BuildingRecord,
    profile: &InteriorProfile,
    space_keys: &std::collections::BTreeMap<String, SpaceId>,
    child_doodad_ids: &mut Vec<crate::world::DoodadId>,
    child_building_ids: &mut Vec<BuildingId>,
) -> Result<(), InteriorError> {
    let layout = world.layout();
    let anchor_global = parent.placement.position.to_global(layout);
    let rotation = parent.placement.rotation;

    for placement in profile.children.iter().filter(|child| child.enabled) {
        let Some(space_id) = resolve_profile_space_key(space_keys, placement.space_key) else {
            continue;
        };
        let global = anchor_global + rotation * placement.local_position;
        let position = WorldPosition::from_global(global, layout);
        match &placement.kind {
            InteriorChildKind::Doodad(definition_id) => {
                if doodad_catalog.get(definition_id).is_none() {
                    return Err(InteriorError::missing_child_definition(
                        placement.key,
                        definition_id,
                    ));
                }
                let created = create_doodad(
                    doodad_catalog,
                    world,
                    definition_id,
                    position,
                    DoodadSource::Authored,
                    DoodadPlacementOverrides {
                        rotation: Some(rotation * placement.local_rotation),
                        ..Default::default()
                    },
                    Some(occupancy),
                )
                .map_err(|err| InteriorError::InteriorSpawnFailed {
                    building_id: parent.id,
                    reason: format!("{err:?}"),
                })?;
                world.mutate_doodad(created.id, |record| {
                    record.metadata.parent_building_id = Some(parent.id);
                    record.metadata.interior_space_id = Some(space_id);
                });
                child_doodad_ids.push(created.id);
            }
            InteriorChildKind::Building(definition_id) => {
                if building_catalog.get(definition_id).is_none() {
                    return Err(InteriorError::MissingChildDefinition {
                        key: placement.key.to_string(),
                        definition: definition_id.as_str().to_string(),
                    });
                }
                let created = create_building(
                    building_catalog,
                    world,
                    definition_id,
                    position,
                    rotation * placement.local_rotation,
                    BuildingSource::Authored,
                    parent.ownership,
                    Some(occupancy),
                )
                .map_err(|err| InteriorError::InteriorSpawnFailed {
                    building_id: parent.id,
                    reason: format!("{err:?}"),
                })?;
                world
                    .mutate_building(created.id, |record| {
                        record.parent_building_id = Some(parent.id);
                        record.interior.interior_space_id = Some(space_id);
                    })
                    .ok_or(InteriorError::ParentBuildingMissing(parent.id))?;
                child_building_ids.push(created.id);
            }
        }
    }
    Ok(())
}

/// Rebuild runtime navigation for an already-activated building after blueprint edits (NV1.5).
pub fn refresh_building_navigation_runtime(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &super::catalog::InteriorProfileCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
) -> Result<(), InteriorError> {
    let record = world
        .get_building(building_id)
        .cloned()
        .ok_or(InteriorError::ParentBuildingMissing(building_id))?;
    if !record.interior.activated {
        return Ok(());
    }

    let definition = building_catalog.get(&record.definition_id).ok_or_else(|| {
        InteriorError::InteriorSpawnFailed {
            building_id,
            reason: format!("missing definition `{}`", record.definition_id.as_str()),
        }
    })?;

    // Optional: a blueprint-only building has no profile and still refreshes.
    let profile_key = record
        .interior
        .profile_id
        .clone()
        .or_else(|| definition.interior_profile_id.clone());
    let profile = profile_key
        .as_deref()
        .and_then(|key| interior_catalog.get(&InteriorProfileId::new(key)));

    let resolved = resolve_building_navigation_blueprint(
        definition,
        nav_catalog,
        record.interior.navigation_blueprint_override.as_ref(),
    )
    .map_err(|err| InteriorError::InteriorSpawnFailed {
        building_id,
        reason: err.to_string(),
    })?;

    let Some(resolved) = resolved else {
        return Err(InteriorError::InteriorSpawnFailed {
            building_id,
            reason: "no navigation blueprint available to refresh".into(),
        });
    };

    let layout = world.layout();
    let blueprint = resolved.blueprint();
    world.space_registry_mut().remove_building(building_id);
    world
        .building_navigation_runtime_mut()
        .remove_building(building_id);

    let spaces = blueprint_space_templates(blueprint);
    let portals = blueprint_portal_templates(blueprint);
    let (space_keys, mut portal_keys) = register_building_navigation_profile(
        world.space_registry_mut(),
        &record,
        definition,
        layout,
        &spaces,
        &portals,
    );
    let model = building_model_world_transform(definition, &record.placement, layout);
    world
        .building_navigation_runtime_mut()
        .insert(build_navigation_runtime(
            building_id,
            blueprint,
            model,
            &space_keys,
            &portal_keys,
        ));
    if let Some(profile) = profile {
        supplement_door_portals_from_profile(
            world.space_registry_mut(),
            &record,
            definition,
            layout,
            profile,
            &space_keys,
            &mut portal_keys,
        )?;

        for template in &profile.doors {
            let Some(portal_id) = portal_keys.get(template.portal_key).copied() else {
                continue;
            };
            for door_id in world.door_store().building_door_ids(building_id).to_vec() {
                let Some(door) = world.door_store_mut().get_mut(door_id) else {
                    continue;
                };
                if door.definition_key == template.key {
                    door.portal_id = portal_id;
                    DoorStore::sync_portal_enabled(world, door_id)?;
                }
            }
        }
    }

    let space_ids: Vec<String> = world
        .space_registry()
        .building_space_ids(building_id)
        .iter()
        .map(|space_id| space_id.raw().to_string())
        .collect();
    world.mutate_building(building_id, |building| {
        building.spaces.space_ids = space_ids;
    });

    let mut outcome = InteriorActivationOutcome::new(
        building_id,
        record.definition_id.clone(),
        InteriorActivationStatus::Refreshed,
    );
    outcome.blueprint_id = Some(blueprint.id.clone());
    outcome.profile_id = profile.map(|profile| profile.id.clone());
    outcome.blueprint_authority = classify_blueprint_authority(
        definition,
        nav_catalog,
        record.interior.navigation_blueprint_override.as_ref(),
    );
    populate_runtime_counts(world, building_id, &mut outcome);
    record_outcome(world, outcome);

    Ok(())
}

/// Remove interior runtime state when parent building is destroyed or removed.
pub fn deactivate_building_interior(
    world: &mut WorldData,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    occupancy: Option<OccupancyCatalogs<'_>>,
    building_id: BuildingId,
) -> Result<(), InteriorError> {
    let record = world
        .get_building(building_id)
        .cloned()
        .ok_or(InteriorError::ParentBuildingMissing(building_id))?;
    if !record.interior.activated {
        return Ok(());
    }

    for raw in &record.interior.child_doodad_ids {
        let doodad_id = crate::world::DoodadId::new(*raw);
        let _ = crate::world::remove_doodad(world, doodad_id, occupancy);
    }
    for raw in &record.interior.child_building_ids {
        let child_id = BuildingId::new(*raw);
        let _ = crate::world::remove_building(
            world,
            child_id,
            occupancy,
            Some(building_catalog),
            Some(doodad_catalog),
            None,
            None,
        );
    }

    world.door_store_mut().remove_building(building_id);
    world.space_registry_mut().remove_building(building_id);
    world
        .building_navigation_runtime_mut()
        .remove_building(building_id);

    world.mutate_building(building_id, |building| {
        building.spaces.space_ids.clear();
        building.interior = BuildingInteriorState::default();
    });
    world.interior_activation_outcomes_mut().remove(building_id);
    let _ = building_catalog;
    Ok(())
}
