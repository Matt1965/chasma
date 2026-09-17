//! Shared unit presentation inputs decoupled from authoritative [`WorldData`] (CG3).

mod appearance;
mod preview;

pub use appearance::{
    UnitPresentationAppearance, sync_live_unit_presentation_appearance,
};
pub use preview::{
    UnitEditorPreviewFraming, UnitEditorPreviewRoot, UnitEditorPreviewUnit,
    propagate_preview_render_layers,
};
