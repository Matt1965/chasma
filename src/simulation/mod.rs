//! Simulation execution control layer (ADR-046).
//!
//! Separates real-time presentation from authoritative simulation ticks.
//! [`SimulationClock`] (ADR-064) schedules fixed 30 Hz ticks from render delta.
//! Pause gates [`SimulationSystems`] only — rendering, UI, and debug overlays
//! continue on real time.

mod building_params;
mod catalog_params;
mod control;
mod input;
mod plugin;
mod report;
mod tick;

pub use building_params::BuildingSimulationParams;
pub use catalog_params::SimulationCatalogParams;
pub use control::{
    FrameTickPlan, MAX_SIMULATION_TICKS_PER_FRAME, SIMULATION_TICK_SECONDS, SimulationClock,
    SimulationControlRequests, SimulationControlState, apply_simulation_control_requests,
};
pub use input::handle_simulation_keyboard;
pub use plugin::{SimulationControlSystems, SimulationPlugin, SimulationSystems};
pub use report::SimulationTickReport;
pub use tick::run_simulation_tick;
