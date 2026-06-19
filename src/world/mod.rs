use bevy::prelude::*;

mod biome;
mod chunk;
mod config;
mod coordinates;
mod data;
mod doodad;
mod terrain;

pub use biome::{
    BiomeColorEntry, BiomeColorMapping, BiomeId, BiomeImportError, BiomeMask, BiomeMaskBounds,
    BiomeSample, import_biome_mask_from_png, import_biome_mask_from_png_bytes,
};
#[cfg(any(test, feature = "dev"))]
pub use biome::{
    dev_biome_mask_bounds, log_dev_biome_load_outcome, try_load_default_dev_biome_mask,
    try_load_dev_biome_mask, biome_mask_path_for_world, DevBiomeLoadOutcome, DevBiomeLoadSummary,
    DEV_BIOME_MASK_PATH, DEV_SOURCE_WORLD_DIR,
};
pub use chunk::{ChunkData, ChunkId};
pub use config::WorldConfig;
pub use coordinates::{ChunkCoord, ChunkLayout, LocalPosition, WorldPosition};
pub use data::{ChunkExtent, WorldData};
pub use doodad::{
    create_doodad, filter_candidates_by_biome, filter_candidates_by_exclusion_zones,
    chunk_needs_procedural_materialization, filter_candidates_by_terrain, finalize_placements,
    generate_chunk_doodads, generate_chunk_doodads_with_settings, lookup_doodad,
    materialize_candidates, materialize_candidates_with_exclusion, materialize_candidates_with_options,
    move_doodad, remove_doodad, try_materialize_procedural_chunk_doodads, ChunkDoodadStore,
    ChunkProceduralMaterializeOutcome,
    DoodadAuthoringError, DoodadCatalog, DoodadCatalogError, DoodadDefinition,
    DoodadDefinitionId, DoodadExclusionZone, DoodadGenerationContext, DoodadGenerationSettings,
    DoodadId, DoodadInsertError, DoodadKind, DoodadMaterializationReport, DoodadMetadata,
    DoodadPlacement, DoodadPlacementOverrides, DoodadRecord, DoodadRenderKey, DoodadSource,
    DoodadSpawnCandidate, BiomeFilterResult, ExclusionFilterOptions, ExclusionFilterResult,
    FinalizedDoodadPlacement, MaterializationOptions, PlacementFinalizationResult,
    ProceduralDoodadKey, TerrainValidationResult, starter_definitions,
};
pub use terrain::{Heightfield, TerrainDataError, TerrainMask, TerrainMetadata};
#[cfg(feature = "terrain-import")]
pub use terrain::{
    DecodeError, GaeaImportError, ImportError, SourceHeightfield, chunk_data_from_source_tile,
    decode_exr_heightfield, expected_chunk_samples_per_edge, gaea_color_dir, gaea_height_dir,
    import_gaea_tile_directory, import_world, parse_gaea_export_filename, source_tile_samples_per_edge,
    validate_gaea_tile_dimensions,
};

/// Owns the World Data Layer: the authoritative coordinate model (ADR-001),
/// chunk identity and definitions (ADR-002), terrain data (ADR-003, ADR-008),
/// doodad data (ADR-015), doodad type catalog (ADR-016), biome mask authority
/// (ADR-024), and world configuration.
///
/// This is the lowest architectural layer; every later layer depends on it. It
/// registers the foundational data types for reflection and initializes the
/// [`WorldConfig`] and (empty) [`WorldData`] resources. It owns no terrain
/// import, rendering, or systems in this phase (ROADMAP Phase 1).
pub struct WorldFoundationPlugin;

impl Plugin for WorldFoundationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ChunkCoord>()
            .register_type::<LocalPosition>()
            .register_type::<WorldPosition>()
            .register_type::<ChunkLayout>()
            .register_type::<ChunkId>()
            .register_type::<WorldConfig>()
            .register_type::<Heightfield>()
            .register_type::<TerrainMetadata>()
            .register_type::<TerrainMask>()
            .register_type::<ChunkData>()
            .register_type::<ChunkExtent>()
            .register_type::<DoodadId>()
            .register_type::<DoodadKind>()
            .register_type::<DoodadPlacement>()
            .register_type::<DoodadSource>()
            .register_type::<DoodadMetadata>()
            .register_type::<DoodadRecord>()
            .register_type::<ChunkDoodadStore>()
            .register_type::<DoodadExclusionZone>()
            .register_type::<DoodadDefinitionId>()
            .register_type::<DoodadRenderKey>()
            .register_type::<DoodadDefinition>()
            .register_type::<DoodadCatalog>()
            .register_type::<BiomeId>()
            .register_type::<BiomeSample>()
            .register_type::<BiomeColorEntry>()
            .register_type::<BiomeColorMapping>()
            .register_type::<BiomeMaskBounds>()
            .register_type::<BiomeMask>()
            .register_type::<WorldData>();

        app.init_resource::<WorldConfig>();
        app.init_resource::<DoodadCatalog>();
        app.init_resource::<WorldData>();
    }
}
