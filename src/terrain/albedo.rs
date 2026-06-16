//! Terrain albedo presentation types (ADR-009, ADR-011, ADR-013).
//!
//! Albedo grids are authored presentation data. They flow through the decode /
//! materialization pipeline as optional sidecar payloads and are **not** stored
//! in [`crate::world::WorldData`].

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::{ChunkData, ChunkId};

/// Row-major linear RGB samples for one chunk tile (ADR-011 albedo sidecar).
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkAlbedoGrid {
    pub samples_per_edge: usize,
    pub data: Vec<[f32; 3]>,
}

impl ChunkAlbedoGrid {
    /// Build a grid from flat row-major RGB triples.
    ///
    /// Returns an error when `samples_per_edge` is zero, the sample count does
    /// not match `samples_per_edge²`, or any channel is non-finite.
    pub fn from_samples(samples_per_edge: usize, data: Vec<[f32; 3]>) -> Result<Self, AlbedoGridError> {
        if samples_per_edge == 0 {
            return Err(AlbedoGridError::InvalidSamplesPerEdge { samples_per_edge });
        }
        let expected = samples_per_edge * samples_per_edge;
        if data.len() != expected {
            return Err(AlbedoGridError::SampleCountMismatch {
                samples_per_edge,
                expected,
                found: data.len(),
            });
        }
        if data.iter().flatten().any(|v| !v.is_finite()) {
            return Err(AlbedoGridError::NonFiniteSample);
        }
        Ok(Self {
            samples_per_edge,
            data,
        })
    }

    /// Validate that this grid matches the heightfield vertex grid.
    pub fn matches_height_samples(&self, height_samples_per_edge: u32) -> bool {
        self.samples_per_edge == height_samples_per_edge as usize
    }

    pub fn sample(&self, row: usize, col: usize) -> [f32; 3] {
        self.data[row * self.samples_per_edge + col]
    }
}

/// Errors while constructing or validating [`ChunkAlbedoGrid`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlbedoGridError {
    InvalidSamplesPerEdge { samples_per_edge: usize },
    SampleCountMismatch {
        samples_per_edge: usize,
        expected: usize,
        found: usize,
    },
    NonFiniteSample,
}

impl core::fmt::Display for AlbedoGridError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidSamplesPerEdge { samples_per_edge } => {
                write!(f, "albedo samples_per_edge must be > 0, got {samples_per_edge}")
            }
            Self::SampleCountMismatch {
                samples_per_edge,
                expected,
                found,
            } => write!(
                f,
                "albedo sample count {found} does not match {samples_per_edge}x{samples_per_edge} ({expected})"
            ),
            Self::NonFiniteSample => write!(f, "albedo samples must be finite"),
        }
    }
}

impl std::error::Error for AlbedoGridError {}

/// Fallback color policy when no albedo sidecar is available (ADR-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbedoFallback {
    /// Height-normalized gradient (debug / preview).
    #[default]
    HeightGradient,
    /// Flat neutral gray.
    Neutral,
}

/// Compute a per-vertex fallback RGB when no albedo sidecar is available.
pub fn fallback_vertex_color(
    height: f32,
    height_min: f32,
    height_max: f32,
    fallback: AlbedoFallback,
) -> [f32; 3] {
    match fallback {
        AlbedoFallback::Neutral => [0.45, 0.45, 0.45],
        AlbedoFallback::HeightGradient => {
            let t = if height_max > height_min {
                ((height - height_min) / (height_max - height_min)).clamp(0.0, 1.0)
            } else {
                0.5
            };
            lerp_rgb([0.20, 0.45, 0.25], [0.65, 0.55, 0.35], t)
        }
    }
}

fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Resident albedo grids for loaded chunks (terrain runtime only; not in [`WorldData`]).
#[derive(Resource, Default)]
pub struct TerrainChunkAlbedo {
    by_chunk: HashMap<ChunkId, ChunkAlbedoGrid>,
}

impl TerrainChunkAlbedo {
    pub fn get(&self, chunk_id: ChunkId) -> Option<&ChunkAlbedoGrid> {
        self.by_chunk.get(&chunk_id)
    }

    pub fn insert(&mut self, chunk_id: ChunkId, grid: ChunkAlbedoGrid) {
        self.by_chunk.insert(chunk_id, grid);
    }

    pub fn remove(&mut self, chunk_id: ChunkId) {
        self.by_chunk.remove(&chunk_id);
    }
}

/// Decode / sync-load pipeline bundle (not stored in [`WorldData`]; tests / import only).
#[cfg(any(test, feature = "terrain-import"))]
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainChunkPayload {
    pub chunk_data: ChunkData,
    pub albedo: Option<ChunkAlbedoGrid>,
}

#[cfg(any(test, feature = "terrain-import"))]
impl TerrainChunkPayload {
    pub fn new(chunk_data: ChunkData, albedo: Option<ChunkAlbedoGrid>) -> Self {
        Self { chunk_data, albedo }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Heightfield, TerrainMetadata};

    #[test]
    fn from_samples_accepts_valid_grid() {
        let grid = ChunkAlbedoGrid::from_samples(2, vec![[1.0, 0.0, 0.0]; 4]).unwrap();
        assert_eq!(grid.samples_per_edge, 2);
        assert_eq!(grid.data.len(), 4);
    }

    #[test]
    fn from_samples_rejects_mismatched_count() {
        assert!(matches!(
            ChunkAlbedoGrid::from_samples(3, vec![[0.0; 3]; 4]),
            Err(AlbedoGridError::SampleCountMismatch { .. })
        ));
    }

    #[test]
    fn payload_carries_optional_albedo() {
        let hf = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        let chunk_data = ChunkData::new(hf, Vec::new());
        let albedo = ChunkAlbedoGrid::from_samples(3, vec![[0.5, 0.5, 0.5]; 9]).unwrap();

        let without = TerrainChunkPayload::new(chunk_data.clone(), None);
        assert!(without.albedo.is_none());

        let with = TerrainChunkPayload::new(chunk_data, Some(albedo.clone()));
        assert_eq!(with.albedo.as_ref(), Some(&albedo));
        assert_eq!(
            with.chunk_data.metadata,
            TerrainMetadata::from_heightfield(&with.chunk_data.heightfield)
        );
    }

    #[test]
    fn fallback_height_gradient_is_deterministic() {
        let low = fallback_vertex_color(0.0, 0.0, 10.0, AlbedoFallback::HeightGradient);
        let high = fallback_vertex_color(10.0, 0.0, 10.0, AlbedoFallback::HeightGradient);
        assert_ne!(low, high);
        assert_eq!(
            fallback_vertex_color(5.0, 0.0, 10.0, AlbedoFallback::HeightGradient),
            fallback_vertex_color(5.0, 0.0, 10.0, AlbedoFallback::HeightGradient),
        );
    }

    #[test]
    fn fallback_neutral_is_flat_gray() {
        let c = fallback_vertex_color(0.0, 0.0, 100.0, AlbedoFallback::Neutral);
        assert_eq!(c, [0.45, 0.45, 0.45]);
    }
}
