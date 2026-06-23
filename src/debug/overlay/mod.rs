//! Unified debug overlay systems (ADR-039 U-UI3).

mod formation_overlay;
mod helpers;
mod intent_overlay;
mod interaction_overlay;
mod path_overlay;
mod selection_overlay;
mod steering_overlay;

pub use formation_overlay::draw_formation_debug_overlay;
pub use intent_overlay::draw_intent_debug_overlay;
pub use interaction_overlay::draw_interaction_debug_overlay;
pub use path_overlay::draw_path_debug_overlay;
pub use selection_overlay::draw_selection_debug_overlay;
pub use steering_overlay::draw_steering_debug_overlay;

use bevy::prelude::*;

/// Debug overlay presentation systems (read-only simulation access).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DebugOverlaySystems;
