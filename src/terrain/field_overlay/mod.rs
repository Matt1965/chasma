//! Terrain field overlay presentation (ADR-103 TF3).

mod components;
mod mesh;
mod state;
mod sync;

pub use components::TerrainFieldOverlayMesh;
pub use mesh::{
    TerrainFieldOverlayAssets, build_field_overlay_mesh, setup_terrain_field_overlay_assets,
};
pub use state::{
    DEFAULT_OVERLAY_OPACITY_BP, MAX_PLAYER_OVERLAY_OPACITY_BP, TerrainFieldAuxiliaryOverlays,
    TerrainOverlaySelection, TerrainOverlayState,
};
pub use sync::{
    TerrainFieldOverlayDiagnostics, cleanup_orphan_field_overlays, despawn_all_field_overlays,
    despawn_field_overlays_for_chunk, sync_terrain_field_overlays,
};

use bevy::prelude::*;

/// Registers terrain field overlay sync systems.
pub struct TerrainFieldOverlayPlugin;

impl Plugin for TerrainFieldOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TerrainOverlayState>()
            .register_type::<TerrainOverlaySelection>()
            .init_resource::<TerrainOverlayState>()
            .init_resource::<TerrainFieldAuxiliaryOverlays>()
            .init_resource::<TerrainFieldOverlayDiagnostics>()
            .add_systems(Startup, setup_terrain_field_overlay_assets)
            .add_systems(
                Update,
                (sync_terrain_field_overlays, cleanup_orphan_field_overlays)
                    .chain()
                    .after(super::lifecycle::TerrainStreamingSystems)
                    .run_if(resource_exists::<super::spawn::TerrainRenderAssets>),
            );
    }
}

#[cfg(test)]
mod tests;
