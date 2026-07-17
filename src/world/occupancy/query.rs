//! Static occupancy geometric queries (ADR-080 B3).

use bevy::prelude::*;

use super::catalog::FootprintCatalog;
use super::cell::{QuantizedRotation, circle_overlap_blocked};
use super::footprint::{agent_overlaps_footprint, effective_building_footprint};
use super::registration::OccupancyCatalogs;
use super::{OccupancyError, OccupancySource, conservative_block_radius_for_kind};
use crate::world::{
    BuildingCatalog, BuildingRecord, ChunkCoord, DoodadCatalog, DoodadRecord, WorldData,
    WorldPosition, default_blocks_movement,
};

/// Result of a static occupancy overlap query.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticOccupancyResult {
    pub blocked: bool,
    pub source: Option<OccupancySource>,
    pub error: Option<OccupancyError>,
}

/// Authoritative static occupancy query with structured diagnostics.
///
/// Uses footprint geometry against [`WorldData`] records — never render entities.
pub fn query_static_occupancy_at(
    world: &WorldData,
    catalogs: OccupancyCatalogs<'_>,
    position: WorldPosition,
    agent_radius_meters: f32,
) -> StaticOccupancyResult {
    if !(agent_radius_meters >= 0.0) || !agent_radius_meters.is_finite() {
        return StaticOccupancyResult {
            blocked: true,
            source: None,
            error: Some(OccupancyError::InvalidBlockingRadius {
                radius_meters: agent_radius_meters,
            }),
        };
    }

    let layout = world.layout();
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);

    let mut chunks: Vec<ChunkCoord> = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            chunks.push(ChunkCoord::new(
                position.chunk.x + dx,
                position.chunk.z + dz,
            ));
        }
    }
    chunks.sort_by_key(|coord| (coord.x, coord.z));

    let mut first_error: Option<OccupancyError> = None;

    for chunk_coord in chunks {
        let chunk_id = crate::world::ChunkId::new(chunk_coord);

        if let Some(store) = world.buildings_in_chunk(chunk_id) {
            for record in store.records() {
                match building_overlap(
                    catalogs.building,
                    catalogs.footprint,
                    record,
                    layout,
                    center_xz,
                    agent_radius_meters,
                ) {
                    Ok(true) => {
                        return StaticOccupancyResult {
                            blocked: true,
                            source: Some(OccupancySource::Building(record.id)),
                            error: first_error,
                        };
                    }
                    Ok(false) => {}
                    Err(err) => {
                        first_error.get_or_insert(err);
                    }
                }
            }
        }

        if let Some(store) = world.doodads_in_chunk(chunk_id) {
            for record in store.records() {
                match doodad_overlap(
                    catalogs.doodad,
                    record,
                    layout,
                    center_xz,
                    agent_radius_meters,
                ) {
                    Ok(true) => {
                        return StaticOccupancyResult {
                            blocked: true,
                            source: Some(OccupancySource::Doodad(record.id)),
                            error: first_error,
                        };
                    }
                    Ok(false) => {}
                    Err(err) => {
                        first_error.get_or_insert(err.clone());
                        if matches!(err, OccupancyError::MissingDoodadDefinition { .. })
                            && default_blocks_movement(record.kind)
                        {
                            let radius = conservative_block_radius_for_kind(record.kind);
                            let doodad_global = record.placement.position.to_global(layout);
                            let doodad_xz = Vec2::new(doodad_global.x, doodad_global.z);
                            if circle_overlap_blocked(
                                center_xz,
                                doodad_xz,
                                agent_radius_meters,
                                radius,
                            ) {
                                return StaticOccupancyResult {
                                    blocked: true,
                                    source: Some(OccupancySource::Doodad(record.id)),
                                    error: Some(err),
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    StaticOccupancyResult {
        blocked: false,
        source: None,
        error: first_error,
    }
}

/// Fail-closed bool helper for movement/navigation hot paths.
pub fn is_position_blocked_by_static_occupancy(
    world: &WorldData,
    catalogs: OccupancyCatalogs<'_>,
    position: WorldPosition,
    agent_radius_meters: f32,
) -> bool {
    let result = query_static_occupancy_at(world, catalogs, position, agent_radius_meters);
    result.blocked || result.error.is_some()
}

fn building_overlap(
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    record: &BuildingRecord,
    layout: crate::world::ChunkLayout,
    agent_center: Vec2,
    agent_radius: f32,
) -> Result<bool, OccupancyError> {
    if !record.lifecycle_state.blocks_movement() {
        return Ok(false);
    }
    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| OccupancyError::MissingBuildingDefinition(record.definition_id.clone()))?;
    let shape = effective_building_footprint(definition, footprint_catalog)?;
    let rotation = QuantizedRotation::from_quat(record.placement.rotation)?;
    let anchor_global = record.placement.position.to_global(layout);
    let anchor_xz = Vec2::new(anchor_global.x, anchor_global.z);
    Ok(agent_overlaps_footprint(
        agent_center,
        agent_radius,
        shape.as_ref(),
        anchor_xz,
        rotation,
    ))
}

fn doodad_overlap(
    catalog: &DoodadCatalog,
    record: &DoodadRecord,
    layout: crate::world::ChunkLayout,
    agent_center: Vec2,
    agent_radius: f32,
) -> Result<bool, OccupancyError> {
    let Some(definition) = catalog.get(&record.definition_id) else {
        if !default_blocks_movement(record.kind) {
            return Ok(false);
        }
        return Err(OccupancyError::MissingDoodadDefinition {
            definition_id: record.definition_id.clone(),
        });
    };

    let collision = crate::world::resolve_doodad_collision(record, definition);
    if !collision.blocks_movement {
        return Ok(false);
    }

    let doodad_global = record.placement.position.to_global(layout);
    let doodad_xz = Vec2::new(doodad_global.x, doodad_global.z);
    Ok(crate::world::agent_overlaps_footprint_continuous(
        agent_center,
        agent_radius,
        &collision.shape,
        doodad_xz,
        collision.yaw_radians,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingDefinitionId, BuildingOwnership, BuildingSource, ChunkCoord, ChunkLayout,
        DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, LocalPosition, WorldPosition,
        create_building, create_doodad,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn catalogs() -> (DoodadCatalog, BuildingCatalog, FootprintCatalog) {
        (
            DoodadCatalog::default(),
            BuildingCatalog::default(),
            FootprintCatalog::default(),
        )
    }

    fn occ<'a>(
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

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn migrated_doodad_circle_matches_old_overlap() {
        let (doodad, building, footprint) = catalogs();
        let mut world = WorldData::new(layout());
        create_doodad(
            &doodad,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(50.0, 50.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();
        let tree = doodad.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
        let unit_radius = 0.5;
        let edge = tree.block_radius_meters + unit_radius;
        assert!(is_position_blocked_by_static_occupancy(
            &world,
            occ(&doodad, &building, &footprint),
            pos(50.0 + edge, 50.0),
            unit_radius,
        ));
    }

    #[test]
    fn building_blocks_navigation_position() {
        let (doodad, building, footprint) = catalogs();
        let mut world = WorldData::new(layout());
        create_building(
            &building,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap();
        assert!(is_position_blocked_by_static_occupancy(
            &world,
            occ(&doodad, &building, &footprint),
            pos(50.0, 50.0),
            0.5,
        ));
    }
}
