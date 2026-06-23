//! Local unit movement tick for player-issued orders (ADR-030, ADR-033 U8).

use bevy::prelude::*;

use crate::debug::{ClientFrameIndex, CommandTraceBuffer, PendingSimulationTrace};
use crate::world::{
    step_all_unit_movement, DoodadCatalog, NavigationConfig, UnitCatalog, WorldData,
};

/// Advance authoritative unit movement each frame.
pub fn tick_unit_movement(
    time: Res<Time>,
    mut world: ResMut<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    nav_config: Res<NavigationConfig>,
    mut pending_trace: ResMut<PendingSimulationTrace>,
) {
    let step_report = step_all_unit_movement(
        &mut world,
        &unit_catalog,
        &doodad_catalog,
        &nav_config,
        time.delta_secs(),
    );
    if !step_report.command_resolve.failures.is_empty()
        || !step_report.command_resolve.successes.is_empty()
    {
        pending_trace.resolve = Some(step_report.command_resolve);
    }
}

/// Flush simulation command-resolve traces after movement tick.
pub fn flush_simulation_command_trace(
    mut trace: ResMut<CommandTraceBuffer>,
    mut pending: ResMut<PendingSimulationTrace>,
    frame_index: Res<ClientFrameIndex>,
) {
    if let Some(report) = pending.resolve.take() {
        trace.record_command_resolve(frame_index.0, &report);
    }
}
