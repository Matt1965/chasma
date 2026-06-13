use bevy::prelude::*;

/// World-space center of the active local view (ADR-012, ADR-014).
///
/// This is client-local presentation state, not authoritative world data.
/// Terrain streaming and other view-driven systems may read it. The camera
/// layer writes it indirectly through an app-layer bridge.
#[derive(Debug, Clone, Copy, Resource, Reflect)]
#[reflect(Resource)]
pub struct PrimaryViewFocus {
    pub position: Vec3,
}

impl Default for PrimaryViewFocus {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
        }
    }
}

impl PrimaryViewFocus {
    pub const fn new(position: Vec3) -> Self {
        Self { position }
    }
}
