//! Local unit movement tick for player-issued orders (ADR-030, ADR-033 U8).

use bevy::prelude::*;

use crate::debug::{ClientFrameIndex, CommandTraceBuffer, PendingSimulationTrace};
use crate::simulation::SimulationControlState;
use crate::world::{
    step_all_unit_movement, DoodadCatalog, NavigationConfig, UnitCatalog, WorldData,
};

/// Advance authoritative unit movement each frame when simulation is not paused.
pub fn tick_unit_movement(
    time: Res<Time>,
    mut control: ResMut<SimulationControlState>,
    mut world: ResMut<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    nav_config: Res<NavigationConfig>,
    mut pending_trace: ResMut<PendingSimulationTrace>,
) {
    if !control.begin_tick() {
        return;
    }

    let step_report = step_all_unit_movement(
        &mut world,
        &unit_catalog,
        &doodad_catalog,
        &nav_config,
        time.delta_secs(),
    );
    control.complete_tick();

    if !step_report.command_resolve.failures.is_empty()
        || !step_report.command_resolve.successes.is_empty()
    {
        pending_trace.resolve = Some(step_report.command_resolve);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::SimulationControlState;
    use crate::world::{
        create_unit, starter_unit_definitions, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        Heightfield, LocalPosition, UnitCatalog, UnitDefinitionId, UnitSource, WorldData,
        WorldPosition,
    };

    fn test_world_with_unit() -> (WorldData, UnitCatalog, crate::world::UnitId) {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(chunk, ChunkData::new(heightfield, Vec::new()));
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
        );
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        (world, catalog, unit_id)
    }

    #[test]
    fn paused_gate_skips_movement_without_partial_execution() {
        let (mut world, catalog, unit_id) = test_world_with_unit();
        let doodad_catalog = crate::world::DoodadCatalog::default();
        let nav_config = crate::world::NavigationConfig::default();
        let before = world
            .get_unit(unit_id)
            .unwrap()
            .placement
            .position;

        let mut control = SimulationControlState {
            paused: true,
            ..Default::default()
        };
        assert!(!control.begin_tick());

        let _ = step_all_unit_movement(
            &mut world,
            &catalog,
            &doodad_catalog,
            &nav_config,
            1.0 / 60.0,
        );

        let after = world
            .get_unit(unit_id)
            .unwrap()
            .placement
            .position;
        assert_eq!(before, after);
        assert_eq!(control.current_tick, 0);
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
