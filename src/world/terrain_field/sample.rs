//! Structured availability for terrain field queries (ADR-101).

use bevy::prelude::*;

use super::id::TerrainFieldId;
use crate::world::{ChunkCoord, ChunkId};

/// Why a terrain field sample is or is not available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum FieldAvailability {
    Available,
    FieldDefinitionMissing,
    FieldDisabled,
    FieldLayerMissing,
    TileMissing,
    TileNotResident,
    OutsideWorld,
    CorruptTile,
    InvalidCoordinate,
}

impl FieldAvailability {
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Provenance of an effective field sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum FieldSampleSource {
    Base,
    /// Future modifier composition seam (TF1 returns Base only).
    BaseWithModifier,
}

/// Result of a point terrain field query.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TerrainFieldSample {
    pub field_id: TerrainFieldId,
    pub availability: FieldAvailability,
    pub value: u16,
    pub source: FieldSampleSource,
    pub chunk: Option<ChunkCoord>,
    pub tile_revision: Option<u64>,
}

impl TerrainFieldSample {
    pub fn unavailable(field_id: TerrainFieldId, availability: FieldAvailability) -> Self {
        Self {
            field_id,
            availability,
            value: 0,
            source: FieldSampleSource::Base,
            chunk: None,
            tile_revision: None,
        }
    }

    pub fn available(
        field_id: TerrainFieldId,
        value: u16,
        chunk: ChunkCoord,
        tile_revision: u64,
    ) -> Self {
        Self {
            field_id,
            availability: FieldAvailability::Available,
            value,
            source: FieldSampleSource::Base,
            chunk: Some(chunk),
            tile_revision: Some(tile_revision),
        }
    }

    pub fn as_percent(&self) -> Option<f32> {
        if self.availability.is_available() {
            Some(self.value as f32 / 65535.0 * 100.0)
        } else {
            None
        }
    }
}

/// Diagnostic context for interpolation (dev probe).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TerrainFieldInterpolationDebug {
    pub col: u32,
    pub row: u32,
    pub frac_x: u8,
    pub frac_z: u8,
    pub corner_values: [u16; 4],
}

/// Area aggregation report for a deterministic sample region.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TerrainFieldAreaReport {
    pub field_id: TerrainFieldId,
    pub requested_cells: u32,
    pub available_cells: u32,
    pub unavailable_cells: u32,
    pub average: Option<u16>,
    pub minimum: Option<u16>,
    pub maximum: Option<u16>,
    pub usable_coverage: super::basis_points::BasisPoints,
    pub availability: FieldAreaAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum FieldAreaAvailability {
    AllAvailable,
    PartiallyAvailable,
    NoneAvailable,
    EmptyRegion,
}

impl TerrainFieldAreaReport {
    pub fn empty_region(field_id: TerrainFieldId) -> Self {
        Self {
            field_id,
            requested_cells: 0,
            available_cells: 0,
            unavailable_cells: 0,
            average: None,
            minimum: None,
            maximum: None,
            usable_coverage: super::basis_points::BasisPoints::ZERO,
            availability: FieldAreaAvailability::EmptyRegion,
        }
    }
}

/// Chunk context attached to samples when useful for dev diagnostics.
pub type FieldChunkContext = (ChunkId, u64);
