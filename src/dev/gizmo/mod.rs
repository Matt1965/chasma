//! Dev transform gizmos — on-screen translate/rotate/scale controls (ADR-099 DT3).

mod commit;
mod drag;
mod draw;
mod handles;
mod input;
mod math;
mod pick;
mod preview;
mod snap;
mod state;
mod tool;

#[cfg(all(test, feature = "dev"))]
mod tests;

pub use input::{handle_gizmo_keyboard, handle_gizmo_mouse, selected_object, sync_gizmo_target};
pub use preview::{
    DevTransformPreview, apply_building_transform_preview, apply_doodad_transform_preview,
};
pub use state::TransformEditState;
pub use tool::{DevTool, DevToolState, GizmoCoordinateSpace, SelectedWorldObject};

pub use draw::draw_transform_gizmo;
