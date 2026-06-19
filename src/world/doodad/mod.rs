//! Authoritative doodad data types (ADR-015, Phase 3A).
//!
//! Doodads are world data owned by [`crate::world::WorldData`], parallel to
//! terrain [`crate::world::ChunkData`]. Type definitions live in
//! [`catalog::DoodadCatalog`] (ADR-016). No rendering, ECS entities, or runtime
//! systems in this module.

mod authoring;
mod catalog;
mod exclusion;
mod generation;
mod id;
mod kind;
mod materialization;
mod metadata;
mod placement;
mod procedural_key;
mod record;
mod source;
mod store;
mod terrain_validation;

pub use authoring::{
    create_doodad, lookup_doodad, move_doodad, remove_doodad, DoodadAuthoringError,
    DoodadPlacementOverrides,
};
pub use catalog::{
    DoodadCatalog, DoodadCatalogError, DoodadDefinition, DoodadDefinitionId, DoodadRenderKey,
    starter_definitions,
};
pub use exclusion::{
    filter_candidates_by_exclusion_zones, DoodadExclusionZone, ExclusionFilterOptions,
    ExclusionFilterResult,
};
pub use generation::{
    generate_chunk_doodads, generate_chunk_doodads_with_settings, DoodadGenerationContext,
    DoodadGenerationSettings, DoodadSpawnCandidate,
};
pub use id::DoodadId;
pub use kind::DoodadKind;
pub use materialization::{
    materialize_candidates, materialize_candidates_with_exclusion,
    materialize_candidates_with_options, DoodadMaterializationReport, MaterializationOptions,
};
pub use metadata::DoodadMetadata;
pub use placement::{
    finalize_placements, DoodadPlacement, FinalizedDoodadPlacement, PlacementFinalizationResult,
};
pub use procedural_key::ProceduralDoodadKey;
pub use record::DoodadRecord;
pub use source::DoodadSource;
pub use store::ChunkDoodadStore;
pub use terrain_validation::{filter_candidates_by_terrain, TerrainValidationResult};

/// Why [`crate::world::WorldData::insert_doodad`] or relocation rejected a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoodadInsertError {
    /// [`DoodadRecord::placement`] chunk does not match the target [`crate::world::ChunkId`].
    ChunkPlacementMismatch,
    /// No doodad with the given id exists in world data.
    DoodadNotFound,
}
