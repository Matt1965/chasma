use bevy::prelude::*;

use super::catalog::{InteriorChildKind, InteriorProfile};
use super::door::DoorRecord;
use super::door_store::DoorStore;
use super::error::InteriorError;
use super::id::InteriorProfileId;
use crate::world::building::catalog::{BuildingCatalog, BuildingDefinition};
use crate::world::building::navigation_blueprint::{
    blueprint_portal_templates, blueprint_space_templates, build_navigation_runtime,
    register_building_navigation_profile, resolve_building_navigation_blueprint,
    BuildingNavigationBlueprintCatalog,
};
use crate::world::building::record::BuildingRecord;
use crate::world::building::state::BuildingInteriorState;
use crate::world::building::state::BuildingLifecycleState;
use crate::world::{
    BuildingId, BuildingSource, DoodadCatalog, DoodadPlacementOverrides, DoodadSource,
    OccupancyCatalogs, PortalId, PortalRecord, SpaceId, WorldData, WorldPosition,
    building_model_world_transform, create_building, create_doodad, register_building_space_profile,
};

/// Activate authored interior spaces, doors, and child objects when a building completes.
pub fn activate_building_interior(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &super::catalog::InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    nav_catalog: Option<&BuildingNavigationBlueprintCatalog>,
    building_id: BuildingId,
    profile_id: &InteriorProfileId,
) -> Result<(), InteriorError> {
    let record = world
        .get_building(building_id)
        .cloned()
        .ok_or(InteriorError::ParentBuildingMissing(building_id))?;
    if record.interior.activated && !world.door_store().building_door_ids(building_id).is_empty() {
        return Err(InteriorError::BuildingInteriorAlreadyActive(building_id));
    }
    let skip_children = !record.interior.child_doodad_ids.is_empty()
        || !record.interior.child_building_ids.is_empty();
    let profile = interior_catalog
        .get(profile_id)
        .ok_or_else(|| InteriorError::MissingInteriorProfile(profile_id.clone()))?;

    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| InteriorError::InteriorSpawnFailed {
            building_id,
            reason: format!("missing definition `{}`", record.definition_id.as_str()),
        })?;

    let layout = world.layout();
    let blueprint = nav_catalog.and_then(|catalog| {
        resolve_building_navigation_blueprint(
            definition,
            catalog,
            record.interior.navigation_blueprint_override.as_ref(),
        )
        .ok()
        .flatten()
    });

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
        world.building_navigation_runtime_mut().insert(build_navigation_runtime(
            building_id,
            blueprint,
            model,
            &keys.0,
        ));
        let (space_keys, mut portal_keys) = keys;
        supplement_door_portals_from_profile(
            world.space_registry_mut(),
            &record,
            definition,
            layout,
            profile,
            &space_keys,
            &mut portal_keys,
        )?;
        (space_keys, portal_keys)
    } else {
        register_building_space_profile(
            world.space_registry_mut(),
            &record,
            layout,
            &profile.spaces,
            &profile.portals,
        )
    };

    let mut door_ids = Vec::new();
    for template in &profile.doors {
        let portal_id = portal_keys
            .get(template.portal_key)
            .copied()
            .ok_or_else(|| InteriorError::InvalidDoorPortal {
                door_key: template.key.to_string(),
                portal_key: template.portal_key.to_string(),
            })?;
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

    let space_ids: Vec<String> = world
        .space_registry()
        .building_space_ids(building_id)
        .iter()
        .map(|space_id| space_id.raw().to_string())
        .collect();

    let preserved_override = record.interior.navigation_blueprint_override.clone();

    world.mutate_building(building_id, |building| {
        building.spaces.space_ids = space_ids;
        building.interior = BuildingInteriorState {
            profile_id: Some(profile_id.as_str().to_string()),
            navigation_blueprint_override: preserved_override,
            door_ids: door_ids.iter().map(|id| id.raw()).collect(),
            child_doodad_ids: child_doodad_ids.iter().map(|id| id.raw()).collect(),
            child_building_ids: child_building_ids.iter().map(|id| id.raw()).collect(),
            activated: true,
            interior_space_id: None,
        };
    });

    Ok(())
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
) -> Result<(), InteriorError> {
    let Some(profile_key) = definition.interior_profile_id.as_deref() else {
        return Ok(());
    };
    activate_building_interior(
        world,
        building_catalog,
        interior_catalog,
        doodad_catalog,
        occupancy,
        nav_catalog,
        building_id,
        &InteriorProfileId::new(profile_key),
    )
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
        let from_space = *space_keys
            .get(template.from_space_key)
            .ok_or_else(|| InteriorError::InteriorSpawnFailed {
                building_id: building.id,
                reason: format!("missing space `{}`", template.from_space_key),
            })?;
        let to_space = *space_keys
            .get(template.to_space_key)
            .ok_or_else(|| InteriorError::InteriorSpawnFailed {
                building_id: building.id,
                reason: format!("missing space `{}`", template.to_space_key),
            })?;
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
        let space_id = space_keys
            .get(placement.space_key)
            .copied()
            .ok_or_else(|| InteriorError::MissingSpace {
                profile: profile.id.clone(),
                key: placement.space_key.to_string(),
            })?;
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

    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| InteriorError::InteriorSpawnFailed {
            building_id,
            reason: format!("missing definition `{}`", record.definition_id.as_str()),
        })?;

    let profile_key = record
        .interior
        .profile_id
        .as_deref()
        .or(definition.interior_profile_id.as_deref())
        .ok_or_else(|| InteriorError::MissingInteriorProfile(InteriorProfileId::new("missing")))?;
    let profile = interior_catalog
        .get(&InteriorProfileId::new(profile_key))
        .ok_or_else(|| InteriorError::MissingInteriorProfile(InteriorProfileId::new(profile_key)))?;

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
        ));
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

    let space_ids: Vec<String> = world
        .space_registry()
        .building_space_ids(building_id)
        .iter()
        .map(|space_id| space_id.raw().to_string())
        .collect();
    world.mutate_building(building_id, |building| {
        building.spaces.space_ids = space_ids;
    });

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
    world.building_navigation_runtime_mut().remove_building(building_id);

    world.mutate_building(building_id, |building| {
        building.spaces.space_ids.clear();
        building.interior = BuildingInteriorState::default();
    });
    let _ = building_catalog;
    Ok(())
}
