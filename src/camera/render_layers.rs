//! Shared render-layer identifiers for world vs isolated preview rendering (CG3).

use bevy::camera::visibility::RenderLayers;

/// Gameplay world geometry (terrain, units, buildings, environment).
pub const WORLD_RENDER_LAYER: RenderLayers = RenderLayers::layer(0);

/// Unit Editor isolated preview studio (preview unit, lights, backdrop).
pub const PREVIEW_RENDER_LAYER: RenderLayers = RenderLayers::layer(1);
