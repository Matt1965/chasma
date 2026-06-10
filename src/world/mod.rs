use bevy::prelude::*;

mod chunk;
mod config;
mod coordinates;
mod data;
mod terrain;

pub use chunk::{ChunkData, ChunkId};
pub use config::WorldConfig;
pub use coordinates::{ChunkCoord, ChunkLayout, LocalPosition, WorldPosition};
pub use data::{ChunkExtent, WorldData};
pub use terrain::{
    DecodeError, Heightfield, ImportError, MaskSource, SourceHeightfield, TerrainDataError,
    TerrainMask, TerrainMetadata, TerrainSource, decode_exr_heightfield, import_world,
};

/// Owns the World Data Layer: the authoritative coordinate model (ADR-001),
/// chunk identity and definitions (ADR-002), terrain data (ADR-003, ADR-008),
/// and world configuration.
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
            .register_type::<WorldData>();

        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
    }
}
