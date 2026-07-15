use bevy::prelude::*;

use crate::world::{BuildingId, BuildingLifecycleState, ChunkId};

use super::fallback::BuildingFallbackReason;

/// Links a derived render entity to authoritative building data (ADR-079 B2, ADR-095 BA1).
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct BuildingRenderEntity {
    pub building_id: BuildingId,
    pub chunk_id: ChunkId,
    pub lifecycle_state: BuildingLifecycleState,
    /// Active catalog render key used for the current presentation.
    pub active_render_key: Option<String>,
    /// True when showing the diagnostic cuboid instead of a GLB scene.
    pub uses_diagnostic_fallback: bool,
}

/// Marker on building roots that spawned a glTF [`SceneRoot`].
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct BuildingSceneRoot;

/// Marker on diagnostic fallback cuboids.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct BuildingDiagnosticFallback {
    pub reason: BuildingFallbackReason,
}

/// Cached optional scene-node tags discovered once after scene spawn (ADR-095 BA1).
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct BuildingSceneTags {
    pub space_node_names: Vec<String>,
    pub roof_entities: Vec<Entity>,
}

/// Tracks the last lifecycle state tint applied to scene descendants.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuildingLifecycleTintApplied {
    pub lifecycle_state: BuildingLifecycleState,
}
