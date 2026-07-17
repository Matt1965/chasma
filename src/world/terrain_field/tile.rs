//! Per-chunk terrain field sample tile (ADR-101).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::contract::{
    TERRAIN_FIELD_SAMPLE_SPACING_METERS, TERRAIN_FIELD_SAMPLES_PER_EDGE,
    TERRAIN_FIELD_SAMPLES_PER_TILE,
};
use super::error::TerrainFieldStorageError;
use super::id::TerrainFieldId;
use crate::world::{ChunkCoord, ChunkId};

/// Authoritative normalized field samples for one chunk (shared-edge grid).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TerrainFieldTile {
    pub chunk: ChunkCoord,
    pub samples_per_edge: u16,
    pub sample_spacing_meters: f32,
    pub samples: Vec<u16>,
    pub tile_revision: u64,
    pub source_version: String,
}

impl TerrainFieldTile {
    pub fn new_constant(chunk: ChunkCoord, value: u16, source_version: impl Into<String>) -> Self {
        let samples = vec![value; TERRAIN_FIELD_SAMPLES_PER_TILE];
        Self {
            chunk,
            samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
            sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
            samples,
            tile_revision: 1,
            source_version: source_version.into(),
        }
    }

    pub fn validate(&self, field_id: &TerrainFieldId) -> Result<(), TerrainFieldStorageError> {
        if self.samples_per_edge != TERRAIN_FIELD_SAMPLES_PER_EDGE {
            return Err(TerrainFieldStorageError::InvalidSamplesPerEdge {
                found: self.samples_per_edge,
                expected: TERRAIN_FIELD_SAMPLES_PER_EDGE,
            });
        }
        if (self.sample_spacing_meters - TERRAIN_FIELD_SAMPLE_SPACING_METERS).abs() > 1e-4 {
            return Err(TerrainFieldStorageError::InvalidSampleSpacing {
                found: self.sample_spacing_meters,
                expected: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
            });
        }
        if self.samples.len() != TERRAIN_FIELD_SAMPLES_PER_TILE {
            return Err(TerrainFieldStorageError::InvalidTileSampleCount {
                found: self.samples.len(),
                expected: TERRAIN_FIELD_SAMPLES_PER_TILE,
            });
        }
        if self.samples.is_empty() {
            return Err(TerrainFieldStorageError::CorruptTile {
                field_id: field_id.clone(),
                chunk: ChunkId::new(self.chunk),
                reason: "empty sample buffer".to_string(),
            });
        }
        Ok(())
    }

    pub fn sample_index(&self, col: u32, row: u32) -> Option<usize> {
        if col >= self.samples_per_edge as u32 || row >= self.samples_per_edge as u32 {
            return None;
        }
        Some(row as usize * self.samples_per_edge as usize + col as usize)
    }

    pub fn sample_at_vertex(&self, col: u32, row: u32) -> Option<u16> {
        let index = self.sample_index(col, row)?;
        self.samples.get(index).copied()
    }

    pub fn chunk_id(&self) -> ChunkId {
        ChunkId::new(self.chunk)
    }
}
