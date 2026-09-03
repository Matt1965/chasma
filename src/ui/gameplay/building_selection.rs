//! Back-compat alias for dev tools that still reference the old HUD mirror name.
//!
//! Gameplay building menu state lives in [`crate::ui::gameplay::building_panel::BuildingPanelState`].

pub use super::building_panel::BuildingPanelState as GameplayBuildingSelection;
