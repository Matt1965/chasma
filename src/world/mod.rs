use bevy::prelude::*;

mod chunk;
mod config;
mod coordinates;

pub use chunk::ChunkId;
pub use config::WorldConfig;
pub use coordinates::{ChunkCoord, ChunkLayout, LocalPosition, WorldPosition};

/// Owns the World Data Layer foundation: the authoritative coordinate model
/// (ADR-001), chunk identity (ADR-002), and world configuration.
///
/// This is the lowest architectural layer; every later layer depends on it. In
/// Phase 0 it registers the foundational data types for reflection and inserts
/// the [`WorldConfig`] resource. It owns no systems yet (ROADMAP Phase 0).
pub struct WorldFoundationPlugin;

impl Plugin for WorldFoundationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ChunkCoord>()
            .register_type::<LocalPosition>()
            .register_type::<WorldPosition>()
            .register_type::<ChunkLayout>()
            .register_type::<ChunkId>()
            .register_type::<WorldConfig>()
            .init_resource::<WorldConfig>();
    }
}
