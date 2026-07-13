//! World inspector — read-only simulation introspection (ADR-048 U-DEV2).

mod building_actions;
mod capture;
mod input;
mod panel;
mod params;
mod snapshot;
mod state;

pub use building_actions::handle_building_dev_actions;
pub use capture::capture_unit_inspector_snapshot;
pub use input::{DevInspectorUi, handle_inspector_input, refresh_inspector_snapshot};
pub(crate) use panel::{setup_inspector_panel, sync_inspector_panel};
pub use state::WorldInspectorState;
