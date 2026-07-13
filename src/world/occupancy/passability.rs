//! Composed passability aggregation (ADR-080 B3).

use bevy::prelude::*;

use super::OccupancyError;
use super::OccupancySource;
use super::catalog::FootprintCatalog;
use super::query::{is_position_blocked_by_static_occupancy, query_static_occupancy_at};
use super::registration::OccupancyCatalogs;
use crate::world::{
    BuildingCatalog, DoodadCatalog, SlopeWalkability, WorldData, WorldPosition,
    classify_slope_walkability, ground_world_position,
};

/// Catalog bundle for composed passability queries.
#[derive(Debug, Clone, Copy)]
pub struct PassabilityCatalogs<'a> {
    pub doodad: &'a DoodadCatalog,
    pub building: &'a BuildingCatalog,
    pub footprint: &'a FootprintCatalog,
}

impl<'a> PassabilityCatalogs<'a> {
    pub fn occupancy(&self) -> OccupancyCatalogs<'a> {
        OccupancyCatalogs {
            doodad: self.doodad,
            building: self.building,
            footprint: self.footprint,
        }
    }
}

/// Why a position is unavailable (no terrain grounding).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassabilityUnavailableReason {
    TerrainUnavailable,
}

/// Why a position is blocked for movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassabilityBlockReason {
    SlopeTooSteep,
    BuildingOccupied,
    DoodadOccupied,
    CorruptFootprint,
    MissingDefinition,
    InvalidCell,
}

/// Structured passability result.
#[derive(Debug, Clone, PartialEq)]
pub enum PassabilityResult {
    Passable {
        movement_cost_multiplier: f32,
    },
    Blocked {
        reason: PassabilityBlockReason,
        source: Option<OccupancySource>,
    },
    Unavailable {
        reason: PassabilityUnavailableReason,
    },
}

/// Agent parameters for passability evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassabilityAgent {
    pub radius_meters: f32,
    pub max_slope_degrees: f32,
}

impl From<crate::world::NavigationAgent> for PassabilityAgent {
    fn from(agent: crate::world::NavigationAgent) -> Self {
        Self {
            radius_meters: agent.radius_meters,
            max_slope_degrees: agent.max_slope_degrees,
        }
    }
}

/// Deterministic composed passability query.
///
/// Contributor order: terrain availability → slope → static occupancy → (future modifiers).
pub fn query_passability_at(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
) -> PassabilityResult {
    query_passability_in_space(
        world,
        catalogs,
        position,
        agent,
        crate::world::SpaceId::SURFACE,
    )
}

/// Space-scoped passability (ADR-083 B6).
pub fn query_passability_in_space(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
    space_id: crate::world::SpaceId,
) -> PassabilityResult {
    if space_id.is_surface() {
        return query_surface_passability(world, catalogs, position, agent);
    }
    query_interior_passability(world, catalogs, position, agent, space_id)
}

fn query_surface_passability(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
) -> PassabilityResult {
    let Some(grounded) = ground_world_position(world, position) else {
        return PassabilityResult::Unavailable {
            reason: PassabilityUnavailableReason::TerrainUnavailable,
        };
    };

    match classify_slope_walkability(world, grounded, agent.max_slope_degrees) {
        SlopeWalkability::Walkable => {}
        SlopeWalkability::Unavailable => {
            return PassabilityResult::Unavailable {
                reason: PassabilityUnavailableReason::TerrainUnavailable,
            };
        }
        SlopeWalkability::TooSteep => {
            return PassabilityResult::Blocked {
                reason: PassabilityBlockReason::SlopeTooSteep,
                source: None,
            };
        }
    }

    let occupancy =
        query_static_occupancy_at(world, catalogs.occupancy(), grounded, agent.radius_meters);
    if occupancy.blocked {
        let reason = match occupancy.source {
            Some(OccupancySource::Building(_)) => PassabilityBlockReason::BuildingOccupied,
            Some(OccupancySource::Doodad(_)) => PassabilityBlockReason::DoodadOccupied,
            None => PassabilityBlockReason::InvalidCell,
        };
        return PassabilityResult::Blocked {
            reason,
            source: occupancy.source,
        };
    }
    if let Some(error) = occupancy.error {
        return map_occupancy_error(error);
    }

    PassabilityResult::Passable {
        movement_cost_multiplier: 1.0,
    }
}

fn query_interior_passability(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
    space_id: crate::world::SpaceId,
) -> PassabilityResult {
    let _ = catalogs;
    if !(agent.radius_meters >= 0.0) || !agent.radius_meters.is_finite() {
        return PassabilityResult::Blocked {
            reason: PassabilityBlockReason::InvalidCell,
            source: None,
        };
    }
    let layout = world.layout();
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);
    let cell = super::cell::occupancy_cell_at_global_xz(center_xz);
    let chunk = super::cell::chunk_for_occupancy_cell(cell, layout);
    let chunk_id = crate::world::ChunkId::new(chunk);
    if let Some(grid) = world.occupancy_in_chunk(chunk_id) {
        if let Some(entry) = grid.get(cell, space_id.raw()) {
            if matches!(entry.state, super::grid::OccupancyState::Blocked) {
                return PassabilityResult::Blocked {
                    reason: PassabilityBlockReason::BuildingOccupied,
                    source: Some(entry.source),
                };
            }
        }
    }
    PassabilityResult::Passable {
        movement_cost_multiplier: 1.0,
    }
}

fn map_occupancy_error(error: OccupancyError) -> PassabilityResult {
    PassabilityResult::Blocked {
        reason: match error {
            OccupancyError::MissingBuildingDefinition(_)
            | OccupancyError::MissingDoodadDefinition { .. }
            | OccupancyError::MissingFootprint(_) => PassabilityBlockReason::MissingDefinition,
            OccupancyError::InvalidRotation { .. }
            | OccupancyError::InvalidMaskDimensions { .. }
            | OccupancyError::MeshDerivedRequiresFootprintId
            | OccupancyError::DisabledFootprint(_)
            | OccupancyError::CollisionNodeMissing { .. }
            | OccupancyError::BakeFailed(_)
            | OccupancyError::NonFiniteGeometry
            | OccupancyError::OverrideOutOfBounds { .. }
            | OccupancyError::OverrideConflict { .. } => PassabilityBlockReason::CorruptFootprint,
            OccupancyError::OccupancyConflict { .. }
            | OccupancyError::RegistrationIndexMismatch => PassabilityBlockReason::InvalidCell,
            OccupancyError::InvalidBlockingRadius { .. } => PassabilityBlockReason::InvalidCell,
        },
        source: None,
    }
}

/// Thin bool helper — fail-closed on any non-passable result.
pub fn is_position_passable(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
) -> bool {
    matches!(
        query_passability_at(world, catalogs, position, agent),
        PassabilityResult::Passable { .. }
    )
}

/// Legacy-compatible doodad-only check via composed passability.
pub fn is_position_blocked_for_agent(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
) -> bool {
    !is_position_passable(
        world,
        catalogs,
        position,
        PassabilityAgent {
            radius_meters: agent_radius_meters,
            max_slope_degrees,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingDefinitionId, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData, ChunkId,
        ChunkLayout, DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, Heightfield,
        LocalPosition, WorldPosition, create_building, create_doodad,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn catalogs() -> (DoodadCatalog, BuildingCatalog, FootprintCatalog) {
        (
            DoodadCatalog::default(),
            BuildingCatalog::default(),
            FootprintCatalog::default(),
        )
    }

    fn pass<'a>(
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

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn agent() -> PassabilityAgent {
        PassabilityAgent {
            radius_meters: 0.5,
            max_slope_degrees: 40.0,
        }
    }

    #[test]
    fn terrain_and_occupancy_compose() {
        let (doodad, building, footprint) = catalogs();
        let mut world = flat_world();
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
        let result = query_passability_at(
            &world,
            pass(&doodad, &building, &footprint),
            pos(50.0, 50.0),
            agent(),
        );
        assert!(matches!(
            result,
            PassabilityResult::Blocked {
                reason: PassabilityBlockReason::DoodadOccupied,
                ..
            }
        ));
        assert!(matches!(
            query_passability_at(
                &world,
                pass(&doodad, &building, &footprint),
                pos(200.0, 200.0),
                agent()
            ),
            PassabilityResult::Passable { .. }
        ));
    }

    #[test]
    fn occupancy_error_fails_closed() {
        use crate::world::{DoodadId, DoodadKind, DoodadPlacement, DoodadRecord};

        let (doodad, building, footprint) = catalogs();
        let mut world = flat_world();
        let tree_position = pos(20.0, 20.0);
        let far = pos(200.0, 200.0);
        let record = DoodadRecord::new(
            DoodadId::new(99),
            DoodadDefinitionId::new("missing_tree_def"),
            DoodadKind::Tree,
            DoodadPlacement::new(tree_position, Quat::IDENTITY, Vec3::ONE),
            DoodadSource::Authored,
        );
        world
            .insert_doodad(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();
        assert!(!is_position_passable(
            &world,
            pass(&doodad, &building, &footprint),
            far,
            agent()
        ));
    }
}
