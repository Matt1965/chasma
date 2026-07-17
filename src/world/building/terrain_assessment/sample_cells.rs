use bevy::prelude::*;

use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::field_requirement::BuildingFieldRequirementDefinition;
use crate::world::building::placement::BuildingPlacement;
use crate::world::occupancy::scale_footprint_shape;
use crate::world::occupancy::{
    FootprintShape, OccupancyCellCoord, OccupancyError, QuantizedRotation,
    effective_building_footprint_for_placement, occupied_cells_for_footprint_yaw,
};
use crate::world::{ChunkLayout, FootprintCatalog, WorldPosition};

/// Resolve deterministic operational sample cells for one requirement (ADR-104 TF4).
pub fn resolve_building_field_sample_cells(
    building_definition: &BuildingDefinition,
    requirement: &BuildingFieldRequirementDefinition,
    candidate_placement: &BuildingPlacement,
    footprint_catalog: &FootprintCatalog,
    layout: ChunkLayout,
) -> Result<Vec<OccupancyCellCoord>, super::error::TerrainAssessmentError> {
    let shape = resolve_sampling_footprint_shape(
        building_definition,
        requirement,
        footprint_catalog,
        candidate_placement.uniform_scale_f32(),
    )?;
    let anchor_global = candidate_placement.position.to_global(layout);
    let anchor_xz = Vec2::new(anchor_global.x, anchor_global.z);
    let yaw = candidate_placement.rotation.to_euler(EulerRot::YXZ).0;
    let mut cells = occupied_cells_for_footprint_yaw(&shape, anchor_xz, yaw);
    cells.sort_by_key(|cell| (cell.z, cell.x));
    cells.dedup_by_key(|cell| (cell.z, cell.x));
    if cells.is_empty() {
        return Err(super::error::TerrainAssessmentError::SamplingRegionEmpty);
    }
    Ok(cells)
}

fn resolve_sampling_footprint_shape(
    building_definition: &BuildingDefinition,
    requirement: &BuildingFieldRequirementDefinition,
    footprint_catalog: &FootprintCatalog,
    uniform_scale: f32,
) -> Result<FootprintShape, super::error::TerrainAssessmentError> {
    if let Some(footprint_id) = &requirement.sampling_footprint_id {
        return footprint_shape_from_catalog(footprint_catalog, footprint_id, uniform_scale);
    }
    if let Some(footprint_id) = &building_definition.field_sampling_footprint_id {
        return footprint_shape_from_catalog(footprint_catalog, footprint_id, uniform_scale);
    }
    let shape = effective_building_footprint_for_placement(
        building_definition,
        footprint_catalog,
        uniform_scale,
    )
    .map_err(|err| map_footprint_error(err))?;
    Ok(shape.into_owned())
}

fn footprint_shape_from_catalog(
    footprint_catalog: &FootprintCatalog,
    footprint_id: &crate::world::FootprintId,
    uniform_scale: f32,
) -> Result<FootprintShape, super::error::TerrainAssessmentError> {
    let footprint = footprint_catalog.get(footprint_id).ok_or_else(|| {
        super::error::TerrainAssessmentError::OperationalFootprintUnavailable(format!(
            "missing footprint `{}`",
            footprint_id.as_str()
        ))
    })?;
    if !footprint.enabled {
        return Err(
            super::error::TerrainAssessmentError::OperationalFootprintUnavailable(format!(
                "disabled footprint `{}`",
                footprint_id.as_str()
            )),
        );
    }
    footprint.validate().map_err(|err| {
        super::error::TerrainAssessmentError::OperationalFootprintUnavailable(format!("{err:?}"))
    })?;
    let shape = if (uniform_scale - 1.0).abs() < 0.0001 {
        footprint.shape.clone()
    } else {
        scale_footprint_shape(&footprint.shape, uniform_scale)
    };
    Ok(shape)
}

fn map_footprint_error(error: OccupancyError) -> super::error::TerrainAssessmentError {
    super::error::TerrainAssessmentError::OperationalFootprintUnavailable(format!("{error:?}"))
}

/// Convenience for placement-plan occupied cells when no custom sampling footprint applies.
pub fn resolve_default_building_field_sample_cells(
    building_definition: &BuildingDefinition,
    candidate_placement: &BuildingPlacement,
    footprint_catalog: &FootprintCatalog,
    layout: ChunkLayout,
) -> Result<Vec<OccupancyCellCoord>, super::error::TerrainAssessmentError> {
    let shape = effective_building_footprint_for_placement(
        building_definition,
        footprint_catalog,
        candidate_placement.uniform_scale_f32(),
    )
    .map_err(|err| map_footprint_error(err))?;
    let anchor_global = candidate_placement.position.to_global(layout);
    let anchor_xz = Vec2::new(anchor_global.x, anchor_global.z);
    let rotation = QuantizedRotation::from_quat(candidate_placement.rotation)
        .unwrap_or(QuantizedRotation::Deg0);
    let mut cells = crate::world::occupied_cells_for_footprint(shape.as_ref(), anchor_xz, rotation);
    cells.sort_by_key(|cell| (cell.z, cell.x));
    cells.dedup_by_key(|cell| (cell.z, cell.x));
    if cells.is_empty() {
        return Err(super::error::TerrainAssessmentError::SamplingRegionEmpty);
    }
    Ok(cells)
}

#[allow(dead_code)]
pub fn placement_from_grounded_anchor(
    grounded_anchor: WorldPosition,
    rotation: Quat,
) -> BuildingPlacement {
    BuildingPlacement::new(grounded_anchor, rotation)
}
