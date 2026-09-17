//! Reusable Unit Editor and isolated 3D preview (CG3).

mod actions;
mod commit;
mod controls;
mod draft;
mod events;
mod plugin;
mod preview_animation;
mod preview_spawn;
mod preview_studio;
mod screen;
mod session;

#[cfg(test)]
mod tests;

pub use actions::open_unit_editor_for_live_unit;
pub use draft::{EquipmentPreviewLoadout, UnitAppearanceDraft};
pub use events::OpenUnitEditorRequest;
pub use plugin::{UnitEditorPlugin, UnitEditorSystems};
pub use session::{UnitEditorMode, UnitEditorSession};
