//! Terrain Runtime Layer (ADR-010).
//!
//! Owns derived, disposable runtime concerns for terrain: the pre-chunked asset
//! format (ADR-011), delivery-agnostic decoding, the Phase 2A synchronous loader
//! (ADR-012), pure mesh generation (ADR-013), and the render-entity marker. It
//! depends on the World Data Layer (`crate::world`) for authoritative truth and
//! never owns that truth itself.
//!
//! Phase 2A is the smallest complete slice: pre-chunked asset → synchronous load
//! → `WorldData` insert → pure mesh → derived render entity. Streaming, region
//! containers, multi-LOD, masks, and the `AssetLoader` path are deferred to
//! later phases (ADR-010 through ADR-013).

use bevy::prelude::*;

pub mod asset;
pub mod components;
pub mod decode;
pub mod load;
pub mod mesh;
pub mod spawn;

#[cfg(feature = "dev")]
pub mod preview;
#[cfg(feature = "terrain-import")]
pub mod write;

pub use asset::{
    CHUNK_FORMAT_VERSION, ChunkFile, MANIFEST_FORMAT_VERSION, Manifest, ManifestChunk,
    ManifestConfig, TerrainAssetError,
};
pub use components::TerrainChunkMesh;
pub use decode::{decode_chunk, decode_manifest};
pub use load::load_world_from_manifest;
pub use mesh::{ChunkLod, build_chunk_mesh};
pub use spawn::spawn_terrain_render_entities;

#[cfg(feature = "terrain-import")]
pub use write::write_world;

/// Owns the Terrain Runtime Layer.
///
/// In Phase 2A this layer provides mechanism (decode/load/mesh functions and the
/// render-entity marker) rather than policy: it registers the
/// [`TerrainChunkMesh`] type for reflection and does not decide which world to
/// load or when. Triggering a load is the responsibility of the application or
/// the dev preview, keeping hardcoded asset paths and load timing out of the
/// core layer (ADR-010). Streaming systems arrive in Phase 2B.
pub struct TerrainRuntimePlugin;

impl Plugin for TerrainRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TerrainChunkMesh>();
    }
}
