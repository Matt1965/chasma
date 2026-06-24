//! Simulation execution control layer (ADR-046).
//!
//! Separates real-time presentation from authoritative simulation ticks.
//! Pause gates [`SimulationSystems`] only — rendering, UI, and debug overlays
//! continue on real time.

mod control;
mod input;
mod plugin;

pub use control::{
    apply_simulation_control_requests, SimulationControlRequests, SimulationControlState,
};
pub use input::handle_simulation_keyboard;
pub use plugin::{SimulationControlSystems, SimulationPlugin, SimulationSystems};
