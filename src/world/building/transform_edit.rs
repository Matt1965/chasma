//! Authoritative building transform editing (ADR-100 DT4).

use std::collections::{BTreeMap, HashSet};

use bevy::prelude::*;

use super::authoring::BuildingAuthoringError;
use super::catalog::BuildingCatalog;
use super::id::BuildingId;
use super::interior::{
    InteriorChildKind, InteriorProfile, InteriorProfileCatalog, InteriorProfileId,
};
use super::placement::BuildingPlacement;
use super::placement_validation::{
    BuildingPlacementConfig, BuildingPlacementContext, BuildingPlacementRejectReason,
    validate_building_transform_placement,
};
use super::record::BuildingRecord;
use crate::world::TransformEditError as DoodadTransformEditError;
use crate::world::authoring_transform::{
    BuildingTransformSafetyClass, FixedScale, QuantizedOrientation, TransformCapabilities,
};
use crate::world::occupancy::{
    OccupancyCatalogs, OccupancySource, apply_registration_plan, plan_register_building,
    plan_unregister_source, update_building_occupancy,
};
use crate::world::{
    DoodadCatalog, DoodadId, PortalId, SpaceId, TaskCancelReason, TaskState, UnitCatalog, UnitId,
    WorldData, WorldPosition, cancel_unit_task, ground_world_position,
};

/// Full candidate transform for a building edit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildingTransformCandidate {
    pub position: WorldPosition,
    pub orientation: QuantizedOrientation,
    pub uniform_scale: FixedScale,
}

/// Options controlling building transform validation and side effects.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BuildingTransformEditOptions {
    pub allow_overlap: bool,
    pub follow_ground: bool,
    pub bypass_placement_validation: bool,
    pub cancel_dependencies: bool,
}

/// Catalog bundle for building transform commits.
#[derive(Debug, Clone, Copy)]
pub struct BuildingTransformCatalogs<'a> {
    pub building: &'a BuildingCatalog,
    pub footprint: &'a crate::world::FootprintCatalog,
    pub doodad: &'a DoodadCatalog,
    pub interior: &'a InteriorProfileCatalog,
    pub unit: &'a UnitCatalog,
}

/// Successful building transform edit report.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingTransformEditReport {
    pub building_id: BuildingId,
    pub previous_placement: BuildingPlacement,
    pub new_placement: BuildingPlacement,
    pub occupied_cell_count: usize,
    pub warnings: Vec<String>,
}

/// Structured building transform edit failures.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildingTransformEditError {
    BuildingNotFound(BuildingId),
    DefinitionNotFound(crate::world::BuildingDefinitionId),
    UnsupportedTransformCapability {
        message: String,
    },
    InvalidTranslation {
        message: String,
    },
    InvalidRotation {
        message: String,
    },
    InvalidScale {
        message: String,
    },
    ScaleOutOfRange {
        min: f32,
        max: f32,
    },
    InstanceScaleNotAllowed,
    PlacementBlocked {
        reason: BuildingPlacementRejectReason,
    },
    BuildingOccupied {
        unit_ids: Vec<UnitId>,
    },
    ActiveDependencies {
        task_ids: Vec<crate::world::TaskId>,
        reservation_point_keys: Vec<String>,
    },
    Occupancy(crate::world::OccupancyError),
    Interior(super::interior::InteriorError),
    Authoring(BuildingAuthoringError),
    SemanticScaleWarning,
}

impl From<BuildingAuthoringError> for BuildingTransformEditError {
    fn from(value: BuildingAuthoringError) -> Self {
        Self::Authoring(value)
    }
}

impl From<super::interior::InteriorError> for BuildingTransformEditError {
    fn from(value: super::interior::InteriorError) -> Self {
        Self::Interior(value)
    }
}

/// Apply a full building transform atomically (position + orientation + uniform scale).
pub fn update_building_transform(
    world: &mut WorldData,
    catalogs: BuildingTransformCatalogs<'_>,
    building_id: BuildingId,
    candidate: BuildingTransformCandidate,
    options: BuildingTransformEditOptions,
) -> Result<BuildingTransformEditReport, BuildingTransformEditError> {
    let previous_record = world
        .get_building(building_id)
        .cloned()
        .ok_or(BuildingTransformEditError::BuildingNotFound(building_id))?;
    let definition = catalogs
        .building
        .get(&previous_record.definition_id)
        .ok_or(BuildingTransformEditError::DefinitionNotFound(
            previous_record.definition_id.clone(),
        ))?;

    validate_safety_class(definition)?;
    validate_capabilities(definition.transform_safety_class, candidate.orientation)?;

    if let Some(blockers) = active_dependencies(world, building_id) {
        if options.cancel_dependencies {
            cancel_building_dependencies(world, building_id, &blockers);
        } else {
            return Err(BuildingTransformEditError::ActiveDependencies {
                task_ids: blockers.task_ids,
                reservation_point_keys: blockers.reservation_point_keys,
            });
        }
    }

    let occupying = units_occupying_building_spaces(world, building_id);
    if !occupying.is_empty() {
        return Err(BuildingTransformEditError::BuildingOccupied {
            unit_ids: occupying,
        });
    }

    let mut position = candidate.position;
    if options.follow_ground {
        position = ground_position(world, position)?;
    }
    validate_translation(world, position)?;
    validate_scale(definition, candidate.uniform_scale)?;

    let rotation = candidate.orientation.to_quat();
    let new_placement =
        BuildingPlacement::new(position, rotation).with_uniform_scale(candidate.uniform_scale);

    if !options.bypass_placement_validation && !options.allow_overlap {
        let ctx = BuildingPlacementContext {
            world,
            building_catalog: catalogs.building,
            footprint_catalog: catalogs.footprint,
            doodad_catalog: catalogs.doodad,
            unit_catalog: catalogs.unit,
            config: BuildingPlacementConfig::default(),
            player_authorized: true,
        };
        let validation = validate_building_transform_placement(
            &ctx,
            &previous_record.definition_id,
            new_placement,
            building_id,
        );
        if !validation.valid {
            return Err(BuildingTransformEditError::PlacementBlocked {
                reason: validation
                    .primary_reason
                    .unwrap_or(BuildingPlacementRejectReason::OutOfBounds),
            });
        }
    }

    let mut warnings = Vec::new();
    if (candidate.uniform_scale.to_f32() - 1.0).abs() > 0.001 {
        warnings.push(
            "Visual/topological size changed; semantic gameplay values are unchanged.".into(),
        );
    }

    let occupancy = OccupancyCatalogs {
        doodad: catalogs.doodad,
        building: catalogs.building,
        footprint: catalogs.footprint,
    };

    let mut trial = previous_record.clone();
    trial.placement = new_placement;

    let register_plan = plan_register_building(world, occupancy, &trial)
        .map_err(BuildingTransformEditError::Occupancy)?;
    let cell_count = register_plan.register.len();
    let mut plan = plan_unregister_source(world, OccupancySource::Building(building_id));
    plan.register = register_plan.register;

    let old_placement = previous_record.placement;
    apply_building_placement(world, building_id, new_placement, &previous_record)?;

    if let Err(err) = apply_registration_plan(world, &plan) {
        let _ = rollback_building_placement(world, building_id, old_placement, &previous_record);
        return Err(BuildingTransformEditError::Occupancy(err));
    }

    if let Err(err) = update_dependent_topology(
        world,
        catalogs,
        building_id,
        &definition,
        old_placement,
        new_placement,
    ) {
        let _ = rollback_building_transform(
            world,
            occupancy,
            building_id,
            &previous_record,
            old_placement,
        );
        return Err(err);
    }

    let moved = world
        .get_building(building_id)
        .cloned()
        .ok_or(BuildingTransformEditError::BuildingNotFound(building_id))?;

    Ok(BuildingTransformEditReport {
        building_id,
        previous_placement: old_placement,
        new_placement: moved.placement,
        occupied_cell_count: cell_count,
        warnings,
    })
}

struct DependencyBlockers {
    task_ids: Vec<crate::world::TaskId>,
    reservation_point_keys: Vec<String>,
}

fn active_dependencies(world: &WorldData, building_id: BuildingId) -> Option<DependencyBlockers> {
    let mut task_ids = Vec::new();
    for &task_id in world.task_store().building_task_ids(building_id) {
        if let Some(task) = world.task_store().get(task_id) {
            if task.assigned_unit_id.is_some()
                || matches!(task.state, TaskState::Assigned | TaskState::InProgress)
            {
                task_ids.push(task_id);
            }
        }
    }
    let mut reservation_point_keys = Vec::new();
    for reservation in world.task_store().reservations() {
        if reservation.building_id == building_id {
            reservation_point_keys.push(reservation.point_key.clone());
        }
    }
    if task_ids.is_empty() && reservation_point_keys.is_empty() {
        None
    } else {
        Some(DependencyBlockers {
            task_ids,
            reservation_point_keys,
        })
    }
}

fn cancel_building_dependencies(
    world: &mut WorldData,
    building_id: BuildingId,
    blockers: &DependencyBlockers,
) {
    let mut events = Vec::new();
    for &task_id in &blockers.task_ids {
        if let Some(task) = world.task_store().get(task_id) {
            if let Some(unit_id) = task.assigned_unit_id {
                cancel_unit_task(world, unit_id, TaskCancelReason::Invalidated, &mut events);
            }
        }
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            if !matches!(task.state, TaskState::Completed | TaskState::Canceled) {
                task.state = TaskState::Canceled;
            }
            task.assigned_unit_id = None;
            task.reserved_point_key = None;
        }
    }
    for point_key in &blockers.reservation_point_keys {
        if let Some(unit_id) = world
            .task_store()
            .reservation_for_point(building_id, point_key)
        {
            world
                .task_store_mut()
                .release_reservation(building_id, point_key, unit_id);
        }
    }
    let _ = events;
}

fn units_occupying_building_spaces(world: &WorldData, building_id: BuildingId) -> Vec<UnitId> {
    let space_ids: HashSet<SpaceId> = world
        .space_registry()
        .building_space_ids(building_id)
        .iter()
        .copied()
        .collect();
    if space_ids.is_empty() {
        return Vec::new();
    }
    world
        .sorted_unit_ids()
        .into_iter()
        .filter(|unit_id| {
            world
                .get_unit(*unit_id)
                .is_some_and(|unit| space_ids.contains(&unit.current_space_id))
        })
        .collect()
}

fn validate_safety_class(
    definition: &super::catalog::BuildingDefinition,
) -> Result<(), BuildingTransformEditError> {
    if definition.transform_safety_class == BuildingTransformSafetyClass::DecorativeNonNavigable {
        if definition.interior_profile_id.is_some() {
            return Err(BuildingTransformEditError::UnsupportedTransformCapability {
                message: "decorative safety class conflicts with interior profile".into(),
            });
        }
    }
    Ok(())
}

fn validate_capabilities(
    safety: BuildingTransformSafetyClass,
    orientation: QuantizedOrientation,
) -> Result<(), BuildingTransformEditError> {
    let caps = safety.capabilities();
    if caps == TransformCapabilities::NONE {
        return Err(BuildingTransformEditError::UnsupportedTransformCapability {
            message: "building editing disabled".into(),
        });
    }
    if safety == BuildingTransformSafetyClass::Navigable {
        if orientation.pitch_millidegrees != 0 || orientation.roll_millidegrees != 0 {
            return Err(BuildingTransformEditError::InvalidRotation {
                message: "navigable buildings allow yaw rotation only".into(),
            });
        }
        let yaw = orientation.yaw_degrees();
        if crate::world::QuantizedRotation::from_degrees_snapped(yaw).is_err() {
            return Err(BuildingTransformEditError::InvalidRotation {
                message: "navigable building yaw must be quantized to 90°".into(),
            });
        }
    }
    Ok(())
}

fn validate_translation(
    world: &WorldData,
    position: WorldPosition,
) -> Result<(), BuildingTransformEditError> {
    let global = position.to_global(world.layout());
    if !global.is_finite() {
        return Err(BuildingTransformEditError::InvalidTranslation {
            message: "non-finite position".into(),
        });
    }
    Ok(())
}

fn validate_scale(
    definition: &super::catalog::BuildingDefinition,
    scale: FixedScale,
) -> Result<(), BuildingTransformEditError> {
    if !definition.allow_instance_scale && scale != FixedScale::ONE {
        return Err(BuildingTransformEditError::InstanceScaleNotAllowed);
    }
    let value = scale.to_f32();
    if value < definition.min_uniform_instance_scale
        || value > definition.max_uniform_instance_scale
    {
        return Err(BuildingTransformEditError::ScaleOutOfRange {
            min: definition.min_uniform_instance_scale,
            max: definition.max_uniform_instance_scale,
        });
    }
    Ok(())
}

fn ground_position(
    world: &WorldData,
    position: WorldPosition,
) -> Result<WorldPosition, BuildingTransformEditError> {
    ground_world_position(world, position).ok_or(BuildingTransformEditError::InvalidTranslation {
        message: "could not ground position".into(),
    })
}

fn apply_building_placement(
    world: &mut WorldData,
    building_id: BuildingId,
    placement: BuildingPlacement,
    previous: &BuildingRecord,
) -> Result<(), BuildingTransformEditError> {
    if previous.placement.position.chunk != placement.position.chunk {
        world
            .relocate_building(building_id, placement.position)
            .map_err(|err| match err {
                super::insert::BuildingInsertError::ChunkPlacementMismatch => {
                    BuildingTransformEditError::InvalidTranslation {
                        message: "chunk placement mismatch".into(),
                    }
                }
                super::insert::BuildingInsertError::BuildingNotFound => {
                    BuildingTransformEditError::BuildingNotFound(building_id)
                }
            })?;
    }
    world
        .mutate_building(building_id, |record| {
            record.placement = placement;
        })
        .ok_or(BuildingTransformEditError::BuildingNotFound(building_id))?;
    Ok(())
}

fn rollback_building_placement(
    world: &mut WorldData,
    building_id: BuildingId,
    placement: BuildingPlacement,
    previous: &BuildingRecord,
) -> Result<(), BuildingTransformEditError> {
    apply_building_placement(world, building_id, placement, previous)
}

fn rollback_building_transform(
    world: &mut WorldData,
    occupancy: OccupancyCatalogs<'_>,
    building_id: BuildingId,
    previous: &BuildingRecord,
    old_placement: BuildingPlacement,
) -> Result<(), BuildingTransformEditError> {
    apply_building_placement(world, building_id, old_placement, previous)?;
    update_building_occupancy(world, occupancy, previous)
        .map_err(BuildingTransformEditError::Occupancy)?;
    Ok(())
}

fn update_dependent_topology(
    world: &mut WorldData,
    catalogs: BuildingTransformCatalogs<'_>,
    building_id: BuildingId,
    definition: &super::catalog::BuildingDefinition,
    old_placement: BuildingPlacement,
    new_placement: BuildingPlacement,
) -> Result<(), BuildingTransformEditError> {
    let record = world
        .get_building(building_id)
        .cloned()
        .ok_or(BuildingTransformEditError::BuildingNotFound(building_id))?;

    if record.interior.activated {
        if let Some(profile_key) = definition.interior_profile_id.as_deref() {
            let profile = catalogs
                .interior
                .get(&InteriorProfileId::new(profile_key))
                .ok_or_else(|| {
                    super::interior::InteriorError::MissingInteriorProfile(InteriorProfileId::new(
                        profile_key,
                    ))
                })?;
            update_interior_topology_in_place(world, &record, profile, new_placement)?;
        }
    }

    transform_interior_children(
        world,
        &record,
        old_placement,
        new_placement,
        OccupancyCatalogs {
            doodad: catalogs.doodad,
            building: catalogs.building,
            footprint: catalogs.footprint,
        },
        catalogs.interior,
        definition,
    )?;

    Ok(())
}

fn update_interior_topology_in_place(
    world: &mut WorldData,
    record: &BuildingRecord,
    profile: &InteriorProfile,
    placement: BuildingPlacement,
) -> Result<(), BuildingTransformEditError> {
    let layout = world.layout();
    let anchor_global = placement.position.to_global(layout);
    let rotation = placement.rotation;
    let uniform_scale = placement.uniform_scale_f32();

    let space_ids = world
        .space_registry()
        .building_space_ids(record.id)
        .to_vec();
    for (template, &space_id) in profile.spaces.iter().zip(space_ids.iter()) {
        let floor_offset = rotation * Vec3::new(0.0, template.local_floor_y * uniform_scale, 0.0);
        if let Some(space) = world.space_registry_mut().get_space_mut(space_id) {
            space.reference_elevation = template.reference_elevation * uniform_scale;
            space.floor_y_global = anchor_global.y + floor_offset.y;
        }
    }

    let key_to_space = profile_space_key_map(profile, &space_ids);
    let portal_ids: Vec<PortalId> = world
        .space_registry()
        .portals()
        .filter(|(_, portal)| portal.owning_building_id == Some(record.id))
        .map(|(id, _)| *id)
        .collect();

    for template in &profile.portals {
        let from_space = key_to_space
            .get(template.from_space_key)
            .copied()
            .unwrap_or(SpaceId::SURFACE);
        let to_space = key_to_space
            .get(template.to_space_key)
            .copied()
            .unwrap_or(SpaceId::SURFACE);
        let from_local = template.from_local_xz * uniform_scale;
        let to_local = template.to_local_position * uniform_scale;
        let from_global = anchor_global + rotation * Vec3::new(from_local.x, 0.0, from_local.y);
        let to_global = anchor_global + rotation * to_local;
        for portal_id in &portal_ids {
            let Some(portal) = world.space_registry_mut().get_portal_mut(*portal_id) else {
                continue;
            };
            if portal.from_space == from_space && portal.to_space == to_space {
                portal.from_center_global_xz = Vec2::new(from_global.x, from_global.z);
                portal.from_radius_meters = template.from_radius_meters * uniform_scale;
                portal.to_position = WorldPosition::from_global(to_global, layout);
            }
        }
    }

    Ok(())
}

fn profile_space_key_map(
    profile: &InteriorProfile,
    space_ids: &[SpaceId],
) -> BTreeMap<&'static str, SpaceId> {
    let mut map = BTreeMap::from([("surface", SpaceId::SURFACE)]);
    for (template, &space_id) in profile.spaces.iter().zip(space_ids.iter()) {
        map.insert(template.key, space_id);
    }
    map
}

fn transform_interior_children(
    world: &mut WorldData,
    parent: &BuildingRecord,
    old_placement: BuildingPlacement,
    new_placement: BuildingPlacement,
    occupancy: OccupancyCatalogs<'_>,
    interior_catalog: &InteriorProfileCatalog,
    definition: &super::catalog::BuildingDefinition,
) -> Result<(), BuildingTransformEditError> {
    let layout = world.layout();
    let old_anchor = old_placement.position.to_global(layout);
    let new_anchor = new_placement.position.to_global(layout);
    let old_rot = old_placement.rotation;
    let new_rot = new_placement.rotation;
    let old_scale = old_placement.uniform_scale_f32();
    let new_scale = new_placement.uniform_scale_f32();

    let profile = definition
        .interior_profile_id
        .as_deref()
        .and_then(|key| interior_catalog.get(&InteriorProfileId::new(key)));

    for raw in &parent.interior.child_doodad_ids {
        let doodad_id = DoodadId::new(*raw);
        let Some(child) = world.get_doodad(doodad_id).cloned() else {
            continue;
        };
        let new_child_placement = if let Some(profile) = profile.as_ref() {
            child_placement_from_profile(
                profile, &child, parent.id, new_anchor, new_rot, new_scale, layout,
            )?
        } else {
            delta_transform_placement(
                &child.placement.position,
                child.placement.orientation.to_quat(),
                old_anchor,
                old_rot,
                old_scale,
                new_anchor,
                new_rot,
                new_scale,
                layout,
            )
        };
        crate::world::update_doodad_transform(
            world,
            occupancy.doodad,
            doodad_id,
            crate::world::DoodadTransformCandidate {
                position: new_child_placement.0,
                orientation: crate::world::QuantizedOrientation::from_quat(new_child_placement.1)
                    .unwrap_or(crate::world::QuantizedOrientation::IDENTITY),
                scale: child.placement.scale,
            },
            crate::world::DoodadTransformEditOptions {
                bypass_placement_validation: true,
                ..Default::default()
            },
            Some(occupancy),
        )
        .map_err(map_doodad_transform_error)?;
    }

    for raw in &parent.interior.child_building_ids {
        let child_id = BuildingId::new(*raw);
        let Some(child) = world.get_building(child_id).cloned() else {
            continue;
        };
        let (position, rotation) = if let Some(profile) = profile.as_ref() {
            building_child_from_profile(
                profile, &child, parent.id, new_anchor, new_rot, new_scale, layout,
            )?
        } else {
            delta_transform_placement(
                &child.placement.position,
                child.placement.rotation,
                old_anchor,
                old_rot,
                old_scale,
                new_anchor,
                new_rot,
                new_scale,
                layout,
            )
        };
        let child_scale = child.placement.uniform_scale;
        apply_building_placement(
            world,
            child_id,
            BuildingPlacement::new(position, rotation).with_uniform_scale(child_scale),
            &child,
        )?;
        if let Some(updated) = world.get_building(child_id).cloned() {
            update_building_occupancy(world, occupancy, &updated)
                .map_err(BuildingTransformEditError::Occupancy)?;
        }
    }

    Ok(())
}

fn child_placement_from_profile(
    profile: &InteriorProfile,
    child: &crate::world::DoodadRecord,
    parent_id: BuildingId,
    anchor: Vec3,
    rotation: Quat,
    uniform_scale: f32,
    layout: crate::world::ChunkLayout,
) -> Result<(WorldPosition, Quat), BuildingTransformEditError> {
    for placement in profile.children.iter().filter(|c| c.enabled) {
        if let InteriorChildKind::Doodad(def_id) = &placement.kind {
            if def_id == &child.definition_id {
                let local = placement.local_position * uniform_scale;
                let global = anchor + rotation * local;
                let position = WorldPosition::from_global(global, layout);
                let child_rotation = rotation * placement.local_rotation;
                return Ok((position, child_rotation));
            }
        }
    }
    Err(BuildingTransformEditError::UnsupportedTransformCapability {
        message: format!("missing profile placement for doodad child of {parent_id:?}"),
    })
}

fn building_child_from_profile(
    profile: &InteriorProfile,
    child: &BuildingRecord,
    parent_id: BuildingId,
    anchor: Vec3,
    rotation: Quat,
    uniform_scale: f32,
    layout: crate::world::ChunkLayout,
) -> Result<(WorldPosition, Quat), BuildingTransformEditError> {
    for placement in profile.children.iter().filter(|c| c.enabled) {
        if let InteriorChildKind::Building(def_id) = &placement.kind {
            if def_id == &child.definition_id {
                let local = placement.local_position * uniform_scale;
                let global = anchor + rotation * local;
                let position = WorldPosition::from_global(global, layout);
                let child_rotation = rotation * placement.local_rotation;
                return Ok((position, child_rotation));
            }
        }
    }
    Err(BuildingTransformEditError::UnsupportedTransformCapability {
        message: format!("missing profile placement for building child of {parent_id:?}"),
    })
}

fn map_doodad_transform_error(err: DoodadTransformEditError) -> BuildingTransformEditError {
    match err {
        DoodadTransformEditError::OccupancyRegistrationFailed(e) => {
            BuildingTransformEditError::Occupancy(e)
        }
        other => BuildingTransformEditError::UnsupportedTransformCapability {
            message: format!("child doodad update failed: {other:?}"),
        },
    }
}

fn delta_transform_placement(
    old_position: &WorldPosition,
    old_rotation: Quat,
    old_anchor: Vec3,
    old_parent_rot: Quat,
    old_parent_scale: f32,
    new_anchor: Vec3,
    new_parent_rot: Quat,
    new_parent_scale: f32,
    layout: crate::world::ChunkLayout,
) -> (WorldPosition, Quat) {
    let old_global = old_position.to_global(layout);
    let local_offset =
        old_parent_rot.inverse() * (old_global - old_anchor) / old_parent_scale.max(0.001);
    let new_global = new_anchor + new_parent_rot * local_offset * new_parent_scale;
    let new_rotation = new_parent_rot * old_parent_rot.inverse() * old_rotation;
    (WorldPosition::from_global(new_global, layout), new_rotation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, BuildingLifecycleState, BuildingOwnership, BuildingSource, ChunkCoord,
        ChunkLayout, DoodadCatalog, FootprintCatalog, LocalPosition, OccupancyCatalogs,
        create_dev_complete_building,
    };

    fn layout_world() -> WorldData {
        WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn position(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn catalogs<'a>(
        building: &'a BuildingCatalog,
        footprint: &'a FootprintCatalog,
        doodad: &'a DoodadCatalog,
        interior: &'a InteriorProfileCatalog,
        unit: &'a UnitCatalog,
    ) -> BuildingTransformCatalogs<'a> {
        BuildingTransformCatalogs {
            building,
            footprint,
            doodad,
            interior,
            unit,
        }
    }

    #[test]
    fn rejects_non_quantized_navigable_yaw() {
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let interior = InteriorProfileCatalog::default();
        let unit = UnitCatalog::default();
        let mut world = layout_world();
        let record = create_dev_complete_building(
            &building,
            &mut world,
            &crate::world::BuildingDefinitionId::new("hut"),
            position(64.0, 64.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap();
        let orientation = QuantizedOrientation::from_degrees(45.0, 0.0, 0.0).expect("orientation");
        let err = update_building_transform(
            &mut world,
            catalogs(&building, &footprint, &doodad, &interior, &unit),
            record.id,
            BuildingTransformCandidate {
                position: position(64.0, 64.0),
                orientation,
                uniform_scale: FixedScale::ONE,
            },
            BuildingTransformEditOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            BuildingTransformEditError::InvalidRotation { .. }
        ));
    }

    #[test]
    fn empty_building_translation_updates_placement() {
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let interior = InteriorProfileCatalog::default();
        let unit = UnitCatalog::default();
        let mut world = layout_world();
        let record = create_dev_complete_building(
            &building,
            &mut world,
            &crate::world::BuildingDefinitionId::new("hut"),
            position(64.0, 64.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
            Some(OccupancyCatalogs {
                doodad: &doodad,
                building: &building,
                footprint: &footprint,
            }),
        )
        .unwrap();
        let new_pos = position(80.0, 80.0);
        let report = update_building_transform(
            &mut world,
            catalogs(&building, &footprint, &doodad, &interior, &unit),
            record.id,
            BuildingTransformCandidate {
                position: new_pos,
                orientation: QuantizedOrientation::IDENTITY,
                uniform_scale: FixedScale::ONE,
            },
            BuildingTransformEditOptions {
                bypass_placement_validation: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.new_placement.position, new_pos);
        assert_eq!(
            world.get_building(record.id).unwrap().placement.position,
            new_pos
        );
    }
}
