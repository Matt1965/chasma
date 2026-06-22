//! Player / gameplay input layer (ADR-033 U8).
//!
//! Client-local selection and SC2-style move commands. Issues orders against
//! authoritative [`crate::world::WorldData`]; does not store simulation state.

mod indicator;
mod input;
mod pick;
mod plugin;
mod selection;
mod settings;
mod simulation;
mod terrain_click;

pub use indicator::{sync_unit_selection_indicator, UnitSelectionIndicatorState};
pub use pick::{cursor_world_ray, pick_terrain_position_along_ray, pick_unit_along_ray, unit_pick_radius};
pub use plugin::{PlayerControlSystems, PlayerPlugin};
pub use selection::PlayerUnitSelection;
pub use settings::PlayerInteractionSettings;
pub use terrain_click::{authoritative_position_at_global_xz, terrain_click_to_world_position, TerrainClickResult};
