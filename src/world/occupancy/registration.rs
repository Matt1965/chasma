//! Occupancy registration lifecycle (ADR-080 B3).

use bevy::prelude::*;

use super::catalog::FootprintCatalog;
use super::cell::{OccupancyCellCoord, QuantizedRotation, chunk_for_occupancy_cell};
use super::footprint::{
    FootprintShape, effective_building_footprint, occupied_cells_for_footprint,
};
use super::grid::default_space_id;
use super::grid::{ChunkOccupancyGrid, OccupancyCellEntry, OccupancyState};
use super::{OccupancyError, OccupancySource, conservative_block_radius_for_kind};
use crate::world::{
    BuildingCatalog, BuildingId, BuildingLifecycleState, BuildingRecord, ChunkId, DoodadCatalog,
    DoodadId, DoodadKind, DoodadRecord, WorldData, default_blocks_movement,
};

/// Catalog bundle for occupancy registration and queries.
#[derive(Debug, Clone, Copy)]
pub struct OccupancyCatalogs<'a> {
    pub doodad: &'a DoodadCatalog,
    pub building: &'a BuildingCatalog,
    pub footprint: &'a FootprintCatalog,
}

/// Planned occupancy mutations applied atomically.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OccupancyRegistrationPlan {
    pub register: Vec<(ChunkId, OccupancyCellCoord, OccupancyCellEntry)>,
    pub unregister: Vec<(ChunkId, OccupancyCellCoord, u32)>,
}

impl OccupancyRegistrationPlan {
    pub fn clear(&mut self) {
        self.register.clear();
        self.unregister.clear();
    }
}

/// Build a registration plan for a building instance.
pub fn plan_register_building(
    world: &WorldData,
    catalogs: OccupancyCatalogs<'_>,
    record: &BuildingRecord,
) -> Result<OccupancyRegistrationPlan, OccupancyError> {
    let definition = catalogs
        .building
        .get(&record.definition_id)
        .ok_or_else(|| OccupancyError::MissingBuildingDefinition(record.definition_id.clone()))?;

    let shape = effective_building_footprint(definition, catalogs.footprint)?;
    let occupancy_state = occupancy_state_for_building(record.lifecycle_state);
    plan_register_shape(
        world,
        OccupancySource::Building(record.id),
        record.placement.position.to_global(world.layout()),
        record.placement.rotation,
        shape.as_ref(),
        occupancy_state,
    )
}

fn occupancy_state_for_building(lifecycle: BuildingLifecycleState) -> OccupancyState {
    match lifecycle {
        BuildingLifecycleState::Planned | BuildingLifecycleState::Ruins => OccupancyState::Reserved,
        BuildingLifecycleState::Foundation
        | BuildingLifecycleState::InProgress
        | BuildingLifecycleState::Complete
        | BuildingLifecycleState::Destroyed => OccupancyState::Blocked,
    }
}

/// Build a registration plan for a doodad instance.
pub fn plan_register_doodad(
    world: &WorldData,
    catalogs: OccupancyCatalogs<'_>,
    record: &DoodadRecord,
) -> Result<OccupancyRegistrationPlan, OccupancyError> {
    let (blocks, radius) = doodad_blocking_params(record, catalogs.doodad)?;
    if !blocks || radius <= 0.0 {
        return Ok(OccupancyRegistrationPlan::default());
    }
    let shape = FootprintShape::Circle {
        radius_meters: radius,
    };
    plan_register_shape(
        world,
        OccupancySource::Doodad(record.id),
        record.placement.position.to_global(world.layout()),
        record.placement.rotation,
        &shape,
        OccupancyState::Blocked,
    )
}

fn doodad_blocking_params(
    record: &DoodadRecord,
    catalog: &DoodadCatalog,
) -> Result<(bool, f32), OccupancyError> {
    if let Some(definition) = catalog.get(&record.definition_id) {
        return Ok((definition.blocks_movement, definition.block_radius_meters));
    }
    if !default_blocks_movement(record.kind) {
        return Ok((false, 0.0));
    }
    let radius = conservative_block_radius_for_kind(record.kind);
    if !(radius > 0.0) || !radius.is_finite() {
        return Err(OccupancyError::InvalidBlockingRadius {
            radius_meters: radius,
        });
    }
    Err(OccupancyError::MissingDoodadDefinition {
        definition_id: record.definition_id.clone(),
    })
}

fn plan_register_shape(
    world: &WorldData,
    source: OccupancySource,
    anchor_global: Vec3,
    rotation: Quat,
    shape: &FootprintShape,
    occupancy_state: OccupancyState,
) -> Result<OccupancyRegistrationPlan, OccupancyError> {
    let rotation = QuantizedRotation::from_quat(rotation)?;
    let anchor_xz = Vec2::new(anchor_global.x, anchor_global.z);
    let cells = occupied_cells_for_footprint(shape, anchor_xz, rotation);
    let layout = world.layout();
    let space_id = default_space_id();
    let mut plan = OccupancyRegistrationPlan::default();

    for cell in cells {
        let chunk = ChunkId::new(chunk_for_occupancy_cell(cell, layout));
        let entry = OccupancyCellEntry {
            state: occupancy_state,
            source,
            space_id,
        };
        if let Some(existing) = world.occupancy_cell(chunk, cell, space_id) {
            if existing.source != source {
                return Err(OccupancyError::OccupancyConflict {
                    cell_x: cell.x,
                    cell_z: cell.z,
                    existing: existing.source,
                    incoming: source,
                });
            }
            continue;
        }
        plan.register.push((chunk, cell, entry));
    }
    Ok(plan)
}

/// Build unregister plan for a source.
pub fn plan_unregister_source(
    world: &WorldData,
    source: OccupancySource,
) -> OccupancyRegistrationPlan {
    let mut plan = OccupancyRegistrationPlan::default();
    for (chunk_id, grid) in world.occupancy_grids() {
        for (cell, entry) in grid.cells() {
            if entry.source == source {
                plan.unregister.push((*chunk_id, *cell, entry.space_id));
            }
        }
    }
    plan
}

/// Apply a registration plan atomically; rolls back on failure.
pub fn apply_registration_plan(
    world: &mut WorldData,
    plan: &OccupancyRegistrationPlan,
) -> Result<(), OccupancyError> {
    let mut rollback_register: Vec<(ChunkId, OccupancyCellCoord, u32)> = Vec::new();
    let mut rollback_restore: Vec<(ChunkId, OccupancyCellCoord, OccupancyCellEntry)> = Vec::new();

    for (chunk, cell, space_id) in &plan.unregister {
        if let Some(removed) = world.remove_occupancy_cell(*chunk, *cell, *space_id) {
            rollback_restore.push((*chunk, *cell, removed));
        }
    }

    for (chunk, cell, entry) in &plan.register {
        if let Some(previous) = world.insert_occupancy_cell(*chunk, *cell, *entry) {
            for (c, cl, e) in rollback_restore {
                world.insert_occupancy_cell(c, cl, e);
            }
            for (c, cl, sid) in rollback_register {
                world.remove_occupancy_cell(c, cl, sid);
            }
            return Err(OccupancyError::OccupancyConflict {
                cell_x: cell.x,
                cell_z: cell.z,
                existing: previous.source,
                incoming: entry.source,
            });
        }
        rollback_register.push((*chunk, *cell, entry.space_id));
    }

    Ok(())
}

/// Register occupancy for a building after world insert.
pub fn register_building_occupancy(
    world: &mut WorldData,
    catalogs: OccupancyCatalogs<'_>,
    record: &BuildingRecord,
) -> Result<(), OccupancyError> {
    let plan = plan_register_building(world, catalogs, record)?;
    apply_registration_plan(world, &plan)
}

/// Register occupancy for a doodad after world insert.
pub fn register_doodad_occupancy(
    world: &mut WorldData,
    catalogs: OccupancyCatalogs<'_>,
    record: &DoodadRecord,
) -> Result<(), OccupancyError> {
    let plan = plan_register_doodad(world, catalogs, record)?;
    apply_registration_plan(world, &plan)
}

/// Unregister all occupancy cells owned by a source.
pub fn unregister_source_occupancy(world: &mut WorldData, source: OccupancySource) {
    let plan = plan_unregister_source(world, source);
    let _ = apply_registration_plan(world, &plan);
}

/// Atomically move building occupancy.
pub fn update_building_occupancy(
    world: &mut WorldData,
    catalogs: OccupancyCatalogs<'_>,
    record: &BuildingRecord,
) -> Result<(), OccupancyError> {
    let register = plan_register_building(world, catalogs, record)?;
    let mut plan = plan_unregister_source(world, OccupancySource::Building(record.id));
    plan.register = register.register;
    apply_registration_plan(world, &plan)
}

/// Atomically move doodad occupancy.
pub fn update_doodad_occupancy(
    world: &mut WorldData,
    catalogs: OccupancyCatalogs<'_>,
    record: &DoodadRecord,
) -> Result<(), OccupancyError> {
    let register = plan_register_doodad(world, catalogs, record)?;
    let mut plan = plan_unregister_source(world, OccupancySource::Doodad(record.id));
    plan.register = register.register;
    apply_registration_plan(world, &plan)
}

/// Rebuild the occupancy index from authoritative world records.
pub fn rebuild_occupancy_index(
    world: &mut WorldData,
    catalogs: OccupancyCatalogs<'_>,
) -> Result<(), OccupancyError> {
    world.clear_occupancy();
    let mut building_ids: Vec<BuildingId> = world.sorted_building_ids();
    for id in building_ids.drain(..) {
        let record = world
            .get_building(id)
            .ok_or(OccupancyError::RegistrationIndexMismatch)?
            .clone();
        register_building_occupancy(world, catalogs, &record)?;
    }
    let mut doodad_ids: Vec<DoodadId> = world.sorted_doodad_ids();
    for id in doodad_ids.drain(..) {
        let record = world
            .get_doodad(id)
            .ok_or(OccupancyError::RegistrationIndexMismatch)?
            .clone();
        register_doodad_occupancy(world, catalogs, &record)?;
    }
    Ok(())
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

    fn occ_catalogs<'a>(
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
    fn building_registers_occupied_cells() {
        let (doodad, building, footprint) = catalogs();
        let occ = occ_catalogs(&doodad, &building, &footprint);
        let mut world = WorldData::new(layout());
        let record = create_building(
            &building,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            Some(occ),
        )
        .unwrap();
        register_building_occupancy(&mut world, occ, &record).unwrap();
        assert!(world.occupancy_cell_count() > 0);
    }

    #[test]
    fn failed_update_preserves_previous_occupancy() {
        let (doodad, building, footprint) = catalogs();
        let occ = occ_catalogs(&doodad, &building, &footprint);
        let mut world = WorldData::new(layout());
        let record = create_building(
            &building,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            Some(occ),
        )
        .unwrap();
        register_building_occupancy(&mut world, occ, &record).unwrap();
        let before = world.occupancy_cell_count();

        let mut bad = record.clone();
        bad.placement.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let err = update_building_occupancy(&mut world, occ, &bad);
        assert!(err.is_err());
        assert_eq!(world.occupancy_cell_count(), before);
    }

    #[test]
    fn rebuild_matches_incremental_registration() {
        let (doodad, building, footprint) = catalogs();
        let occ = occ_catalogs(&doodad, &building, &footprint);
        let mut incremental = WorldData::new(layout());
        let mut rebuilt = WorldData::new(layout());

        let b = create_building(
            &building,
            &mut incremental,
            &BuildingDefinitionId::new("hut"),
            pos(60.0, 60.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            Some(occ),
        )
        .unwrap();
        register_building_occupancy(&mut incremental, occ, &b).unwrap();

        create_building(
            &building,
            &mut rebuilt,
            &BuildingDefinitionId::new("hut"),
            pos(60.0, 60.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            Some(occ),
        )
        .unwrap();
        rebuild_occupancy_index(&mut rebuilt, occ).unwrap();

        assert_eq!(
            incremental.occupancy_cell_count(),
            rebuilt.occupancy_cell_count()
        );
    }
}
