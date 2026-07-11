//! Terrain constraint validation for procedural candidates (ADR-021, Phase 3G).
//!
//! Filters candidates against resident [`crate::world::ChunkData`] heightfields
//! before materialization. Does not use terrain runtime, meshes, or ECS.

mod filter;

pub use filter::{TerrainValidationResult, filter_candidates_by_terrain};
