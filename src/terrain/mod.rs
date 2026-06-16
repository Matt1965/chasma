//! Terrain Runtime Layer (ADR-010).
//!
//! Owns derived, disposable runtime concerns for terrain: the pre-chunked asset
//! format (ADR-011), delivery-agnostic decoding, synchronous loading/streaming
//! (ADR-012), pure mesh generation (ADR-013), and the render-entity marker. It
//! depends on the World Data Layer (`crate::world`) for authoritative truth and
//! never owns that truth itself.

use bevy::prelude::*;

pub mod asset;
pub mod catalog;
pub mod components;
pub mod decode;
pub mod grace;
pub mod lifecycle;
pub mod load;
pub mod materialize;
pub mod mesh;
pub mod residency;
pub mod spawn;
pub mod streaming;

#[cfg(feature = "dev")]
pub mod perf;
#[cfg(feature = "dev")]
pub mod preview;
#[cfg(feature = "terrain-import")]
pub mod write;

pub use asset::{
    CHUNK_FORMAT_VERSION, ChunkFile, MANIFEST_FORMAT_VERSION, Manifest, ManifestChunk,
    ManifestConfig, TerrainAssetError,
};
pub use catalog::TerrainWorldCatalog;
pub use components::TerrainChunkMesh;
pub use decode::{decode_chunk, decode_manifest};
pub use lifecycle::TerrainStreamingSystems;
pub use load::{load_chunk_from_path, load_world_from_manifest};
pub use materialize::PendingChunkMaterializations;
pub use residency::{ChunkDiscardKind, ChunkResidencyState, ChunkResidencyTracker, discard_chunk_residency};
pub use mesh::{ChunkLod, build_chunk_mesh};
pub use spawn::{
    TerrainRenderAssets, despawn_chunk_meshes, spawn_chunk_mesh, spawn_terrain_render_entities,
};
pub use streaming::TerrainStreamingSettings;

#[cfg(feature = "dev")]
pub use perf::{
    TerrainStreamingFrameSample, TerrainStreamingPerfLatest, TerrainStreamingPerfSettings,
};

#[cfg(feature = "terrain-import")]
pub use write::write_world;

/// Owns the Terrain Runtime Layer.
pub struct TerrainRuntimePlugin;

impl Plugin for TerrainRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TerrainChunkMesh>()
            .register_type::<TerrainStreamingSettings>()
            .init_resource::<TerrainStreamingSettings>()
            .init_resource::<ChunkResidencyTracker>()
            .init_resource::<PendingChunkMaterializations>()
            .init_resource::<grace::JustAppliedGrace>();

        #[cfg(feature = "dev")]
        {
            use perf::{
                TerrainStreamingFrameSample, TerrainStreamingPerfFileLog, TerrainStreamingPerfLatest,
                TerrainStreamingPerfSettings, TerrainStreamingPerfState,
            };
            app.register_type::<TerrainStreamingPerfSettings>()
                .register_type::<TerrainStreamingPerfLatest>()
                .register_type::<TerrainStreamingFrameSample>()
                .init_resource::<TerrainStreamingPerfSettings>()
                .init_resource::<TerrainStreamingPerfState>()
                .init_resource::<TerrainStreamingPerfLatest>()
                .init_resource::<TerrainStreamingPerfFileLog>();
        }

        app.add_systems(
                Update,
                (
                    #[cfg(feature = "dev")]
                    lifecycle::begin_terrain_streaming_perf_frame,
                    lifecycle::stream_terrain_chunks,
                    lifecycle::poll_chunk_materializations,
                    lifecycle::apply_chunk_materializations,
                    lifecycle::unload_terrain_chunks,
                    #[cfg(feature = "dev")]
                    perf::report_terrain_streaming_perf,
                )
                    .chain()
                    .in_set(TerrainStreamingSystems)
                    .run_if(resource_exists::<TerrainWorldCatalog>)
                    .run_if(resource_exists::<TerrainRenderAssets>),
            );
    }
}
