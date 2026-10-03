//! Dynamic unit portraits rendered from authoritative appearance data.

mod cache;
mod components;
mod diagnostics;
mod equipment;
mod framing;
mod pipeline;
mod plugin;
mod resolve;
mod signature;
mod studio;
mod studio_images;
mod ui;

pub use components::UnitPortraitSceneRoot;
pub use plugin::{UnitPortraitPlugin, UnitPortraitSystems, configure_portrait_system_sets};
pub use ui::{SelectedUnitPortraitImage, sync_selected_unit_portrait_ui};
