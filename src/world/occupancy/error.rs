//! Structured occupancy and footprint errors (ADR-080 B3).

use bevy::prelude::*;

use crate::world::{
    BuildingDefinitionId, BuildingId, DoodadDefinitionId, DoodadId, DoodadKind, FootprintId,
};

/// Why footprint resolution or registration failed.
#[derive(Debug, Clone, PartialEq)]
pub enum OccupancyError {
    MissingFootprint(FootprintId),
    DisabledFootprint(FootprintId),
    MissingBuildingDefinition(BuildingDefinitionId),
    MissingDoodadDefinition {
        definition_id: DoodadDefinitionId,
    },
    InvalidRotation {
        yaw_degrees: f32,
    },
    CollisionNodeMissing {
        asset: String,
    },
    BakeFailed(String),
    NonFiniteGeometry,
    InvalidMaskDimensions {
        width_cells: u32,
        depth_cells: u32,
    },
    OverrideOutOfBounds {
        cell_x: i32,
        cell_z: i32,
    },
    OverrideConflict {
        cell_x: i32,
        cell_z: i32,
    },
    OccupancyConflict {
        cell_x: i32,
        cell_z: i32,
        existing: OccupancySource,
        incoming: OccupancySource,
    },
    RegistrationIndexMismatch,
    InvalidBlockingRadius {
        radius_meters: f32,
    },
    MeshDerivedRequiresFootprintId,
}

/// Identity of a static occupancy contributor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum OccupancySource {
    Building(BuildingId),
    Doodad(DoodadId),
}

impl OccupancySource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Building(_) => "building",
            Self::Doodad(_) => "doodad",
        }
    }
}

/// Conservative movement block radius when a blocking doodad definition is missing.
pub fn conservative_block_radius_for_kind(kind: DoodadKind) -> f32 {
    match kind {
        DoodadKind::Tree => 1.0,
        DoodadKind::Rock => 2.5,
        DoodadKind::Ruin => 4.0,
        DoodadKind::ResourceNode => 4.0,
        DoodadKind::Bush => 0.0,
    }
}
