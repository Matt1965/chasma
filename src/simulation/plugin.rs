//! Simulation control plugin registration (ADR-046).

use bevy::prelude::*;

use super::control::{apply_simulation_control_requests, SimulationControlRequests, SimulationControlState};
use super::input::handle_simulation_keyboard;

/// Systems that advance authoritative simulation (WorldData mutation ticks).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SimulationSystems;

/// Simulation pause/resume control (runs before [`SimulationSystems`]).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SimulationControlSystems;

/// Registers simulation control resources and input.
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationControlState>()
            .init_resource::<SimulationControlRequests>()
            .register_type::<SimulationControlState>()
            .configure_sets(
                Update,
                (
                    SimulationControlSystems,
                    SimulationSystems.after(SimulationControlSystems),
                ),
            )
            .add_systems(
                Update,
                (
                    apply_simulation_control_requests,
                    handle_simulation_keyboard,
                )
                    .chain()
                    .in_set(SimulationControlSystems),
            );
    }
}
