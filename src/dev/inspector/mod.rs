//! World inspector — read-only simulation introspection (ADR-048 U-DEV2).

mod building_actions;
mod capture;
mod doodad_actions;
mod doodad_snapshot;
mod input;
mod panel;
mod params;
mod snapshot;
mod state;

pub use building_actions::handle_building_dev_actions;
pub use capture::capture_unit_inspector_snapshot;
pub use doodad_actions::handle_doodad_transform_hotkeys;
pub use input::{DevInspectorUi, handle_inspector_input, refresh_inspector_snapshot};
pub(crate) use panel::{setup_inspector_panel, sync_inspector_panel};
pub use state::WorldInspectorState;
