//! Dev world authoring tools (ADR-044).

use bevy::prelude::App;

mod batch_spawn;
mod brush;
mod pattern;
mod placement_rules;
mod preview;
pub(crate) mod preview_model;
mod preview_visual;

pub use batch_spawn::{BatchSpawnRequest, BatchSpawnScratch, execute_batch_spawn};
pub use brush::{BrushMode, BrushSettings, MAX_BRUSH_SPAWN_COUNT};
pub use placement_rules::{PlacementRejectReason, PlacementRules};
pub use preview::{
    DevPlacementPreview, DevPlacementPreviewScratch, DevPreviewAnchor,
    update_dev_placement_preview,
};
pub use preview_model::DevPlacementModelGhost;

pub(crate) fn init_placement_model_preview(app: &mut App) {
    preview_model::init_dev_placement_model_preview(app);
}
