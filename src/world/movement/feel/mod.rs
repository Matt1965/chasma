//! Movement feel and stabilization (ADR-037 U12).

mod command_buffer;
mod smoothing;
mod stabilization;

pub(crate) use command_buffer::resolve_one;
pub use command_buffer::{
    CommandBufferResolveReport, CommandResolveSuccess, PATH_RESOLVE_BUDGET_PER_TICK,
    PendingUnitOrder, UnitCommandBuffer, start_unit_move_to,
};
pub use smoothing::{MovementSmoothingState, should_skip_direction_smoothing};
pub use stabilization::{
    StabilizedMovementHeading, stabilized_movement_heading, steering_is_allowed,
};

/// Conservative feel tuning (does not alter pathfinding or formation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementFeelSettings {
    pub direction_smooth_factor: f32,
    pub max_smoothed_turn_radians: f32,
}

impl Default for MovementFeelSettings {
    fn default() -> Self {
        Self {
            direction_smooth_factor: 0.18,
            max_smoothed_turn_radians: 20.0_f32.to_radians(),
        }
    }
}

impl MovementFeelSettings {
    pub const DEFAULT: Self = Self {
        direction_smooth_factor: 0.18,
        max_smoothed_turn_radians: 0.34906584,
    };
}
