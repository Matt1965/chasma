//! World XZ bounds for terrain field import (ADR-102).
//!
//! Matches [`crate::world::biome::BiomeMaskBounds`] / ADR-024:
//! - origin is southwest (minimum X, minimum Z)
//! - row 0 / minimum sample index is minimum Z (south)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::biome::BiomeMaskBounds;
use crate::world::{ChunkExtent, ChunkLayout};

/// World-space bounds for mapping field samples to global XZ (ADR-102).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldWorldBounds {
    pub origin_x: f32,
    pub origin_z: f32,
    pub extent_x: f32,
    pub extent_z: f32,
}

impl TerrainFieldWorldBounds {
    pub const fn new(origin_x: f32, origin_z: f32, extent_x: f32, extent_z: f32) -> Self {
        Self {
            origin_x,
            origin_z,
            extent_x,
            extent_z,
        }
    }

    pub fn from_chunk_extent(extent: ChunkExtent, layout: ChunkLayout) -> Self {
        let b = BiomeMaskBounds::from_chunk_extent(extent, layout);
        Self::from_biome_bounds(b)
    }

    pub fn from_biome_bounds(bounds: BiomeMaskBounds) -> Self {
        Self {
            origin_x: bounds.origin_x,
            origin_z: bounds.origin_z,
            extent_x: bounds.extent_x,
            extent_z: bounds.extent_z,
        }
    }

    pub fn to_biome_bounds(self) -> BiomeMaskBounds {
        BiomeMaskBounds::new(self.origin_x, self.origin_z, self.extent_x, self.extent_z)
    }

    pub fn max_x(&self) -> f32 {
        self.origin_x + self.extent_x
    }

    pub fn max_z(&self) -> f32 {
        self.origin_z + self.extent_z
    }

    /// Global position of a target field sample vertex (shared-edge grid).
    pub fn sample_vertex_global(&self, col: u32, row: u32, spacing_meters: f32) -> (f32, f32) {
        (
            self.origin_x + col as f32 * spacing_meters,
            self.origin_z + row as f32 * spacing_meters,
        )
    }
}

/// Target shared-edge sample dimensions for an authored chunk extent.
pub fn target_sample_dimensions(extent: ChunkExtent) -> (u32, u32) {
    let chunks_x = (extent.max.x - extent.min.x + 1) as u32;
    let chunks_z = (extent.max.z - extent.min.z + 1) as u32;
    let intervals = super::super::contract::TERRAIN_FIELD_INTERVALS_PER_CHUNK as u32;
    (chunks_x * intervals + 1, chunks_z * intervals + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ChunkCoord;

    #[test]
    fn two_by_two_chunks_has_65_samples_per_axis() {
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        };
        assert_eq!(target_sample_dimensions(extent), (65, 65));
    }
}
