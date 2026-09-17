//! Simulation water surface and locomotion (gameplay authority).
//!
//! Environment rendering observes [`WorldWaterState`]; it does not own sea level.

mod query;
mod state;

pub use query::{
    LocomotionSurface, cancel_swimming_incompatible_actions, effective_move_speed_mps,
    locomotion_surface_at, refresh_all_unit_locomotion, sample_locomotion_support_height,
    unit_can_perform_normal_actions, unit_is_swimming, unit_locomotion_surface,
    unit_order_requires_normal_actions, water_depth_at, water_skips_seabed_slope,
};
pub use state::{
    DEFAULT_PRESENTATION_WATER_LEVEL, DEFAULT_SWIM_ENTER_VISIBLE_METERS,
    DEFAULT_SWIM_EXIT_VISIBLE_METERS, DEFAULT_SWIM_ORIGIN_OFFSET_VISIBLE_METERS,
    DEFAULT_SWIM_SPEED_MULTIPLIER, WorldWaterState, presentation_to_sim,
    sim_to_presentation,
};

#[cfg(test)]
mod tests;
