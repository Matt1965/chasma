//! Authoritative doodad data types (ADR-015, Phase 3A).
//!
//! Doodads are world data owned by [`crate::world::WorldData`], parallel to
//! terrain [`crate::world::ChunkData`]. Type definitions live in
//! [`catalog::DoodadCatalog`] (ADR-016). No rendering, ECS entities, or runtime
//! systems in this module.

mod authoring;
mod biome_filter;
mod catalog;
mod collision;
mod exclusion;
mod generation;
mod id;
mod kind;
mod materialization;
mod metadata;
mod placement;
mod procedural_key;
mod procgen;
mod record;
#[cfg(any(test, feature = "dev"))]
mod restore;
mod source;
mod store;
mod terrain_validation;
mod transform_edit;

pub use authoring::{
    DoodadAuthoringError, DoodadPlacementOverrides, create_doodad, lookup_doodad, move_doodad,
    remove_doodad,
};
pub use biome_filter::{BiomeFilterResult, filter_candidates_by_biome};
#[cfg(test)]
pub use catalog::starter_definitions;
pub use catalog::{
    DoodadCatalog, DoodadCatalogError, DoodadDefinition, DoodadDefinitionId, DoodadRenderKey,
    default_blocks_movement,
};
pub use collision::{
    DoodadInstanceCollision, doodad_authored_interaction_radius_meters, doodad_composed_xz_scale,
    doodad_definition_placement_radius_meters, doodad_interaction_radius_meters,
    resolve_doodad_collision, resolve_doodad_collision_from_catalog,
    tilted_blocker_projection_warning,
};
pub use exclusion::{
    DoodadExclusionZone, ExclusionFilterOptions, ExclusionFilterResult,
    filter_candidates_by_exclusion_zones,
};
pub use generation::DeterministicRng;
pub use generation::{
    DoodadGenerationContext, DoodadGenerationSettings, DoodadSpawnCandidate,
    generate_chunk_doodads, generate_chunk_doodads_with_settings,
};
pub use id::DoodadId;
pub use kind::DoodadKind;
pub use materialization::{
    DoodadMaterializationReport, MaterializationOptions, materialize_candidates,
    materialize_candidates_with_exclusion, materialize_candidates_with_options,
};
pub use metadata::DoodadMetadata;
pub use placement::{
    DoodadPlacement, FinalizedDoodadPlacement, PlacementFinalizationResult, finalize_placements,
};
pub use procedural_key::ProceduralDoodadKey;
pub use procgen::{
    ChunkProceduralMaterializeOutcome, chunk_needs_procedural_materialization,
    try_materialize_procedural_chunk_doodads,
};
pub use record::DoodadRecord;
#[cfg(any(test, feature = "dev"))]
pub use restore::{DoodadRestoreError, restore_doodad_record, validate_doodad_for_restore};
pub use source::DoodadSource;
pub use store::ChunkDoodadStore;
pub use terrain_validation::{TerrainValidationResult, filter_candidates_by_terrain};
pub use transform_edit::{
    DoodadTransformCandidate, DoodadTransformEditOptions, TransformEditError, TransformEditReport,
    nudge_doodad_position, update_doodad_transform,
};

/// Why [`crate::world::WorldData::insert_doodad`] or relocation rejected a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoodadInsertError {
    /// [`DoodadRecord::placement`] chunk does not match the target [`crate::world::ChunkId`].
    ChunkPlacementMismatch,
    /// No doodad with the given id exists in world data.
    DoodadNotFound,
}
