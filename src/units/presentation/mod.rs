//! Shared unit presentation inputs decoupled from authoritative [`WorldData`] (CG3).

mod appearance;
mod preview;
mod roster_preview;

pub use appearance::{
    UnitPresentationAppearance, sync_live_unit_presentation_appearance,
};
pub use preview::{
    UnitEditorPreviewDressingRoot, UnitEditorPreviewEnvironment, UnitEditorPreviewFraming,
    UnitEditorPreviewGround, UnitEditorPreviewRoot, UnitEditorPreviewUnit,
    propagate_preview_render_layers,
};
pub use roster_preview::{
    ROSTER_STAGE_PRESENTATION_YAW, UnitEditorPreviewRosterMember, roster_preview_offsets,
    roster_stage_layout_offsets, roster_stage_layout_position,
};
