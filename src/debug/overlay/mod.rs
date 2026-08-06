//! Unified debug overlay systems (ADR-039 U-UI3).

mod blueprint_overlay;
mod combat_overlay;
mod formation_overlay;
mod helpers;
mod intent_overlay;
mod interaction_overlay;
mod interior_clearance_overlay;
mod nav_cells;
mod navigation_mask_mesh;
mod navigation_overlay;
mod path_overlay;
mod runtime_entrance_overlay;
mod selection_overlay;
mod steering_overlay;

pub use blueprint_overlay::draw_blueprint_debug_overlay;
pub use combat_overlay::draw_combat_debug_overlay;
pub use formation_overlay::draw_formation_debug_overlay;
pub use intent_overlay::draw_intent_debug_overlay;
pub use interaction_overlay::draw_interaction_debug_overlay;
pub use interior_clearance_overlay::draw_interior_clearance_overlay;
pub use navigation_mask_mesh::{setup_navigation_mask_overlay_assets, sync_navigation_mask_meshes};
pub use navigation_overlay::{
    NavigationMaskCache, NavigationMaskDrawStats, draw_navigation_debug_overlay,
};
pub use path_overlay::draw_path_debug_overlay;
pub use runtime_entrance_overlay::draw_runtime_entrance_overlay;
pub use selection_overlay::draw_selection_debug_overlay;
pub use steering_overlay::draw_steering_debug_overlay;

use bevy::prelude::*;

/// Debug overlay presentation systems (read-only simulation access).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DebugOverlaySystems;
