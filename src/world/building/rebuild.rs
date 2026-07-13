//! Authoritative building-derived index rebuild (ADR-086 B9).

use bevy::prelude::*;

use super::catalog::BuildingCatalog;
use crate::world::{
    DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, OccupancyCatalogs, OccupancyError,
    WorldData, prune_invalid_building_tasks, rebuild_occupancy_index, sync_construction_tasks,
};

/// Why a building world rebuild failed.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildingRebuildError {
    Occupancy(String),
    IndexConsistency(&'static str),
}

impl std::fmt::Display for BuildingRebuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Occupancy(error) => write!(f, "occupancy rebuild failed: {error}"),
            Self::IndexConsistency(reason) => write!(f, "index consistency failed: {reason}"),
        }
    }
}

impl From<OccupancyError> for BuildingRebuildError {
    fn from(value: OccupancyError) -> Self {
        Self::Occupancy(format!("{value:?}"))
    }
}

/// Rebuild all building-derived authoritative indexes deterministically (ADR-086 B9).
///
/// Clears and reconstructs occupancy from records. Door/space graphs are expected to be
/// reconciled separately during scene load (`reconcile_building_interiors_after_scene_load`).
/// Task availability is synced/pruned after occupancy is valid.
pub fn rebuild_building_world_indexes(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    simulation_tick: u64,
) -> Result<(), BuildingRebuildError> {
    let occ = OccupancyCatalogs {
        doodad: doodad_catalog,
        building: building_catalog,
        footprint: footprint_catalog,
    };
    rebuild_occupancy_index(world, occ)?;
    sync_construction_tasks(world, building_catalog, simulation_tick);
    prune_invalid_building_tasks(world);
    #[cfg(any(test, feature = "dev"))]
    world
        .verify_instance_indexes()
        .map_err(BuildingRebuildError::IndexConsistency)?;
    let _ = building_catalog;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingLifecycleState, BuildingOwnership, ChunkCoord, ChunkLayout, LocalPosition,
        OccupancyCatalogs, WorldData, WorldPosition, place_player_building,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn rebuild_matches_incremental_occupancy() {
        let building = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let occ = OccupancyCatalogs {
            building: &building,
            doodad: &doodad,
            footprint: &footprint,
        };
        let mut world = WorldData::new(layout());
        let _ = place_player_building(
            &building,
            &mut world,
            &crate::world::BuildingDefinitionId::new("hut"),
            pos(32.0, 32.0),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            occ,
        );
        let before = world.occupancy_cell_count();
        rebuild_building_world_indexes(&mut world, &building, &footprint, &doodad, 0).unwrap();
        let after = world.occupancy_cell_count();
        assert_eq!(before, after);
        assert!(
            world
                .get_building(world.sorted_building_ids()[0])
                .is_some_and(|record| record.lifecycle_state == BuildingLifecycleState::Planned)
        );
    }
}
