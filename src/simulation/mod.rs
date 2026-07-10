//! Simulation execution control layer (ADR-046).
//!
//! Separates real-time presentation from authoritative simulation ticks.
//! [`SimulationClock`] (ADR-064) schedules fixed 30 Hz ticks from render delta.
//! Pause gates [`SimulationSystems`] only — rendering, UI, and debug overlays
//! continue on real time.

mod control;
mod input;
mod plugin;

pub use control::{
    apply_simulation_control_requests, FrameTickPlan, SimulationClock, SimulationControlRequests,
    SimulationControlState, MAX_SIMULATION_TICKS_PER_FRAME, SIMULATION_TICK_SECONDS,
};
pub use input::handle_simulation_keyboard;
pub use plugin::{SimulationControlSystems, SimulationPlugin, SimulationSystems};
