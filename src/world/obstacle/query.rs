//! Doodad obstacle queries — delegates to generalized occupancy (ADR-031, ADR-080 B3).

use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, DoodadCatalog, DoodadId, FootprintCatalog, OccupancyCatalogs, OccupancySource,
    PassabilityAgent, PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, WorldData,
    WorldPosition, query_passability_at, query_static_occupancy_at,
};

use super::error::ObstacleQueryError;

/// Result of an obstacle overlap query at a world position.
#[derive(Debug, Clone, PartialEq)]
pub struct ObstacleQueryResult {
    pub blocked: bool,
    pub blocking_doodad: Option<DoodadId>,
    pub error: Option<ObstacleQueryError>,
}

fn passability_catalogs<'a>(
    doodad: &'a DoodadCatalog,
    building: &'a BuildingCatalog,
    footprint: &'a FootprintCatalog,
) -> PassabilityCatalogs<'a> {
    PassabilityCatalogs {
        doodad,
        building,
        footprint,
    }
}

fn occupancy_catalogs<'a>(
    doodad: &'a DoodadCatalog,
    building: &'a BuildingCatalog,
    footprint: &'a FootprintCatalog,
) -> OccupancyCatalogs<'a> {
    OccupancyCatalogs {
        doodad,
        building,
        footprint,
    }
}

fn map_error(error: crate::world::OccupancyError) -> ObstacleQueryError {
    match error {
        crate::world::OccupancyError::MissingDoodadDefinition { definition_id } => {
            ObstacleQueryError::MissingDoodadDefinition { definition_id }
        }
        crate::world::OccupancyError::InvalidBlockingRadius { radius_meters } => {
            ObstacleQueryError::InvalidBlockingRadius { radius_meters }
        }
        other => ObstacleQueryError::Occupancy(other),
    }
}

/// Authoritative obstacle query with structured diagnostics.
pub fn query_obstacle_at_position(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> ObstacleQueryResult {
    let result = query_static_occupancy_at(
        world,
        occupancy_catalogs(doodad_catalog, building_catalog, footprint_catalog),
        position,
        radius_meters,
    );
    ObstacleQueryResult {
        blocked: result.blocked,
        blocking_doodad: match result.source {
            Some(OccupancySource::Doodad(id)) => Some(id),
            _ => None,
        },
        error: result.error.map(map_error),
    }
}

/// Return `true` when a unit footprint at `position` overlaps static occupancy.
pub fn is_position_blocked_by_doodads(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> bool {
    let catalogs = passability_catalogs(doodad_catalog, building_catalog, footprint_catalog);
    !matches!(
        query_passability_at(
            world,
            catalogs,
            position,
            PassabilityAgent {
                radius_meters,
                max_slope_degrees: f32::MAX,
            },
        ),
        PassabilityResult::Passable { .. }
    )
}

/// The first blocking doodad id overlapping `position`, if any (deterministic order).
pub fn blocking_doodad_at_position(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> Option<DoodadId> {
    let result = query_obstacle_at_position(
        world,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        position,
        radius_meters,
    );
    if result.blocked {
        result.blocking_doodad
    } else {
        None
    }
}

/// Map passability block reason to legacy movement semantics.
pub fn passability_blocks_movement(result: &PassabilityResult) -> bool {
    match result {
        PassabilityResult::Passable { .. } => false,
        PassabilityResult::Unavailable { .. } => true,
        PassabilityResult::Blocked { reason, .. } => {
            matches!(
                reason,
                PassabilityBlockReason::DoodadOccupied
                    | PassabilityBlockReason::BuildingOccupied
                    | PassabilityBlockReason::MissingDefinition
                    | PassabilityBlockReason::CorruptFootprint
                    | PassabilityBlockReason::InvalidCell
            )
        }
    }
}

#[cfg(test)]
include!("query_tests.rs");
