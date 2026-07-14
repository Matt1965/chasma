use bevy::prelude::*;

use super::catalog::{InteriorChildKind, InteriorProfile};
use super::door::DoorRecord;
use super::door_store::DoorStore;
use super::error::InteriorError;
use super::id::InteriorProfileId;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::record::BuildingRecord;
use crate::world::building::state::BuildingInteriorState;
use crate::world::{
    BuildingId, BuildingSource, DoodadCatalog, DoodadPlacementOverrides, DoodadSource,
    OccupancyCatalogs, PortalId, SpaceId, WorldData, WorldPosition, create_building, create_doodad,
    register_building_space_profile,
};

/// Activate authored interior spaces, doors, and child objects when a building completes.
pub fn activate_building_interior(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &super::catalog::InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
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

    let layout = world.layout();
    let (space_keys, portal_keys) = register_building_space_profile(
        world.space_registry_mut(),
        &record,
        layout,
        &profile.spaces,
        &profile.portals,
    );

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

    world.mutate_building(building_id, |building| {
        building.spaces.space_ids = space_ids;
        building.interior = BuildingInteriorState {
            profile_id: Some(profile_id.as_str().to_string()),
            door_ids: door_ids.iter().map(|id| id.raw()).collect(),
            child_doodad_ids: child_doodad_ids.iter().map(|id| id.raw()).collect(),
            child_building_ids: child_building_ids.iter().map(|id| id.raw()).collect(),
            activated: true,
            interior_space_id: None,
        };
    });

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

    world.mutate_building(building_id, |building| {
        building.spaces.space_ids.clear();
        building.interior = BuildingInteriorState::default();
    });
    let _ = building_catalog;
    Ok(())
}
