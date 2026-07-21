//! Authoritative doodad transform editing (ADR-098 DT2).

use bevy::prelude::*;

use super::authoring::DoodadAuthoringError;
use super::catalog::DoodadCatalog;
use super::collision::{resolve_doodad_collision, tilted_blocker_projection_warning};
use super::id::DoodadId;
use super::placement::DoodadPlacement;
use super::record::DoodadRecord;
use crate::world::authoring_transform::{
    AuthoringScale, QuantizedOrientation, TransformCapabilities,
    AUTHORING_INSTANCE_SCALE_MAX, AUTHORING_INSTANCE_SCALE_MIN,
};
use crate::world::occupancy::{
    DoodadRegistrationOptions, OccupancyCatalogs, apply_registration_plan,
    occupied_cells_for_footprint_yaw, plan_register_doodad, plan_unregister_source,
};
use crate::world::{DoodadDefinition, WorldData, WorldPosition, ground_world_position};

/// Full candidate transform for a doodad edit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoodadTransformCandidate {
    pub position: WorldPosition,
    pub orientation: QuantizedOrientation,
    pub scale: AuthoringScale,
}

/// Options controlling transform edit validation and side effects.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DoodadTransformEditOptions {
    pub allow_overlap: bool,
    pub follow_ground: bool,
    pub bypass_placement_validation: bool,
    /// Dev authoring: validate against [`AUTHORING_INSTANCE_SCALE_*`] instead of
    /// definition procgen variation bounds (`min_scale` / `max_scale`).
    pub bypass_definition_scale_range: bool,
}

/// Successful transform edit report.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformEditReport {
    pub doodad_id: DoodadId,
    pub previous_placement: DoodadPlacement,
    pub new_placement: DoodadPlacement,
    pub occupied_cell_count: usize,
    pub warnings: Vec<String>,
}

/// Structured transform edit failures.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformEditError {
    DoodadNotFound(DoodadId),
    DefinitionNotFound(crate::world::DoodadDefinitionId),
    UnsupportedTransformCapability { message: String },
    InvalidTranslation { message: String },
    InvalidRotation { message: String },
    InvalidScale { message: String },
    ScaleOutOfRange { min: f32, max: f32 },
    PlacementBlocked { message: String },
    InvalidGroundSupport,
    CollisionTransformFailed { message: String },
    OccupancyRegistrationFailed(crate::world::OccupancyError),
    ScaleCollapsesFootprint,
    LocationIndexMismatch,
    Authoring(DoodadAuthoringError),
}

impl From<DoodadAuthoringError> for TransformEditError {
    fn from(value: DoodadAuthoringError) -> Self {
        Self::Authoring(value)
    }
}

/// Apply a full doodad transform atomically (position + orientation + scale).
pub fn update_doodad_transform(
    world: &mut WorldData,
    catalog: &DoodadCatalog,
    doodad_id: DoodadId,
    candidate: DoodadTransformCandidate,
    options: DoodadTransformEditOptions,
    occupancy: Option<OccupancyCatalogs<'_>>,
) -> Result<TransformEditReport, TransformEditError> {
    let previous = world
        .get_doodad(doodad_id)
        .cloned()
        .ok_or(TransformEditError::DoodadNotFound(doodad_id))?;
    let definition =
        catalog
            .get(&previous.definition_id)
            .ok_or(TransformEditError::DefinitionNotFound(
                previous.definition_id.clone(),
            ))?;

    validate_capabilities()?;
    let mut position = candidate.position;
    if options.follow_ground {
        position = ground_position(world, position)?;
    }
    validate_translation(world, position)?;
    validate_scale(definition, candidate.scale, &options)?;

    let new_placement = DoodadPlacement::new(position, candidate.orientation, candidate.scale);
    let mut warnings = Vec::new();
    if let Some(w) = tilted_blocker_projection_warning(&DoodadRecord {
        placement: new_placement,
        ..previous.clone()
    }) {
        warnings.push(w);
    }

    let mut trial = previous.clone();
    trial.placement = new_placement;

    if let Some(catalogs) = occupancy {
        let reg_opts = DoodadRegistrationOptions {
            allow_overlap: options.allow_overlap,
        };
        let register_plan = plan_register_doodad(world, catalogs, &trial, reg_opts)
            .map_err(TransformEditError::OccupancyRegistrationFailed)?;
        if register_plan.register.is_empty()
            && resolve_doodad_collision(&trial, definition).blocks_movement
            && !options.bypass_placement_validation
        {
            return Err(TransformEditError::ScaleCollapsesFootprint);
        }
        let mut plan =
            plan_unregister_source(world, crate::world::OccupancySource::Doodad(doodad_id));
        plan.register = register_plan.register;

        let moved = relocate_or_mutate(world, doodad_id, new_placement)?;
        if let Err(err) = apply_registration_plan(world, &plan) {
            let _ = relocate_or_mutate(world, doodad_id, previous.placement);
            return Err(TransformEditError::OccupancyRegistrationFailed(err));
        }

        let cell_count = occupied_cell_count(world, catalog, &moved);
        return Ok(TransformEditReport {
            doodad_id,
            previous_placement: previous.placement,
            new_placement: moved.placement,
            occupied_cell_count: cell_count,
            warnings,
        });
    }

    let moved = relocate_or_mutate(world, doodad_id, new_placement)?;
    let cell_count = occupied_cell_count(world, catalog, &moved);

    Ok(TransformEditReport {
        doodad_id,
        previous_placement: previous.placement,
        new_placement: moved.placement,
        occupied_cell_count: cell_count,
        warnings,
    })
}

fn validate_capabilities() -> Result<(), TransformEditError> {
    let caps = TransformCapabilities::doodad();
    if !caps.translate_x || !caps.rotate_z || !caps.nonuniform_scale {
        return Err(TransformEditError::UnsupportedTransformCapability {
            message: "doodad capability policy incomplete".into(),
        });
    }
    Ok(())
}

fn validate_translation(
    world: &WorldData,
    position: WorldPosition,
) -> Result<(), TransformEditError> {
    let global = position.to_global(world.layout());
    if !global.is_finite() {
        return Err(TransformEditError::InvalidTranslation {
            message: "non-finite position".into(),
        });
    }
    Ok(())
}

fn validate_scale(
    definition: &DoodadDefinition,
    scale: AuthoringScale,
    options: &DoodadTransformEditOptions,
) -> Result<(), TransformEditError> {
    let (min, max) = if options.bypass_definition_scale_range {
        (AUTHORING_INSTANCE_SCALE_MIN, AUTHORING_INSTANCE_SCALE_MAX)
    } else {
        (definition.min_scale, definition.max_scale)
    };
    let vec = scale.to_vec3();
    for component in [vec.x, vec.y, vec.z] {
        if component < min || component > max {
            return Err(TransformEditError::ScaleOutOfRange { min, max });
        }
    }
    if !definition.allow_nonuniform_instance_scale {
        let dx = (vec.x - vec.y).abs();
        let dz = (vec.z - vec.y).abs();
        if dx > 0.01 || dz > 0.01 {
            return Err(TransformEditError::InvalidScale {
                message: "non-uniform scale not allowed by definition".into(),
            });
        }
    }
    Ok(())
}

fn ground_position(
    world: &WorldData,
    position: WorldPosition,
) -> Result<WorldPosition, TransformEditError> {
    ground_world_position(world, position).ok_or(TransformEditError::InvalidGroundSupport)
}

fn relocate_or_mutate(
    world: &mut WorldData,
    id: DoodadId,
    placement: DoodadPlacement,
) -> Result<DoodadRecord, TransformEditError> {
    let current = world
        .get_doodad(id)
        .ok_or(TransformEditError::DoodadNotFound(id))?;
    let needs_relocate = current.placement.position.chunk != placement.position.chunk;
    if needs_relocate {
        world
            .relocate_doodad(id, placement.position)
            .map_err(|_| TransformEditError::LocationIndexMismatch)?;
    }
    world
        .mutate_doodad(id, |record| {
            record.placement = placement;
        })
        .ok_or(TransformEditError::DoodadNotFound(id))
}

fn occupied_cell_count(world: &WorldData, catalog: &DoodadCatalog, record: &DoodadRecord) -> usize {
    let Some(definition) = catalog.get(&record.definition_id) else {
        return 0;
    };
    let collision = resolve_doodad_collision(record, definition);
    if !collision.blocks_movement {
        return 0;
    }
    let global = record.placement.position.to_global(world.layout());
    let anchor_xz = Vec2::new(global.x, global.z);
    occupied_cells_for_footprint_yaw(&collision.shape, anchor_xz, collision.yaw_radians).len()
}

/// Convenience: nudge position by delta meters.
pub fn nudge_doodad_position(
    world: &mut WorldData,
    catalog: &DoodadCatalog,
    doodad_id: DoodadId,
    delta: Vec3,
    options: DoodadTransformEditOptions,
    occupancy: Option<OccupancyCatalogs<'_>>,
) -> Result<TransformEditReport, TransformEditError> {
    let record = world
        .get_doodad(doodad_id)
        .ok_or(TransformEditError::DoodadNotFound(doodad_id))?;
    let layout = world.layout();
    let global = record.placement.position.to_global(layout) + delta;
    let position = WorldPosition::from_global(global, layout);
    update_doodad_transform(
        world,
        catalog,
        doodad_id,
        DoodadTransformCandidate {
            position,
            orientation: record.placement.orientation,
            scale: record.placement.scale,
        },
        options,
        occupancy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        AuthoringScale, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
        DoodadDefinitionId, DoodadKind, DoodadRecord, DoodadSource, Heightfield, LocalPosition,
        QuantizedOrientation,
    };

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            world.insert(
                ChunkId::new(ChunkCoord::new(x, z)),
                ChunkData::new(heightfield.clone(), Vec::new()),
            );
        }
        world
    }

    fn pos(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
        WorldPosition::new(ChunkCoord::new(chunk_x, chunk_z), LocalPosition::new(local))
    }

    fn seed_doodad(world: &mut WorldData, position: WorldPosition) -> DoodadId {
        let record = DoodadRecord::new(
            DoodadId::new(1),
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            DoodadPlacement::identity_at(position),
            DoodadSource::Dev,
        );
        let chunk = ChunkId::new(position.chunk);
        world.insert_doodad(chunk, record).unwrap();
        DoodadId::new(1)
    }

    #[test]
    fn nudge_position_updates_record() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world();
        let id = seed_doodad(&mut world, pos(0, 0, Vec3::new(10.0, 0.0, 10.0)));
        let occ = OccupancyCatalogs {
            doodad: &catalog,
            building: &crate::world::BuildingCatalog::default(),
            footprint: &crate::world::FootprintCatalog::default(),
        };
        let report = nudge_doodad_position(
            &mut world,
            &catalog,
            id,
            Vec3::new(1.0, 0.0, 0.0),
            DoodadTransformEditOptions::default(),
            Some(occ),
        )
        .unwrap();
        let global = report.new_placement.position.to_global(world.layout());
        assert!((global.x - 11.0).abs() < 0.05);
    }

    #[test]
    fn rejects_zero_scale_components() {
        assert!(AuthoringScale::from_non_uniform_f32(0.0, 1.0, 1.0).is_err());
    }

    #[test]
    fn applies_valid_non_uniform_scale() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world();
        let id = seed_doodad(&mut world, pos(0, 0, Vec3::new(0.0, 0.0, 0.0)));
        let position = world.get_doodad(id).unwrap().placement.position;
        let occ = OccupancyCatalogs {
            doodad: &catalog,
            building: &crate::world::BuildingCatalog::default(),
            footprint: &crate::world::FootprintCatalog::default(),
        };
        let result = update_doodad_transform(
            &mut world,
            &catalog,
            id,
            DoodadTransformCandidate {
                position,
                orientation: QuantizedOrientation::IDENTITY,
                scale: AuthoringScale::from_non_uniform_f32(1.1, 1.0, 0.9).unwrap(),
            },
            DoodadTransformEditOptions {
                allow_overlap: true,
                bypass_placement_validation: true,
                ..Default::default()
            },
            Some(occ),
        );
        assert!(result.is_ok(), "{:?}", result.err());
        assert!((world.get_doodad(id).unwrap().placement.scale_vec3().x - 1.1).abs() < 0.02);
    }

    #[test]
    fn cross_chunk_relocate_preserves_id() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world();
        let id = seed_doodad(&mut world, pos(0, 0, Vec3::new(200.0, 0.0, 10.0)));
        let occ = OccupancyCatalogs {
            doodad: &catalog,
            building: &crate::world::BuildingCatalog::default(),
            footprint: &crate::world::FootprintCatalog::default(),
        };
        let report = update_doodad_transform(
            &mut world,
            &catalog,
            id,
            DoodadTransformCandidate {
                position: pos(1, 0, Vec3::new(10.0, 0.0, 10.0)),
                orientation: QuantizedOrientation::IDENTITY,
                scale: AuthoringScale::uniform_one(),
            },
            DoodadTransformEditOptions::default(),
            Some(occ),
        )
        .unwrap();
        assert_eq!(report.doodad_id, id);
        assert_eq!(report.new_placement.position.chunk, ChunkCoord::new(1, 0));
        world.assert_doodad_index_consistent();
    }
}
