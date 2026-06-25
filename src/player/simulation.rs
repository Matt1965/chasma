//! Local unit movement tick for player-issued orders (ADR-030, ADR-033 U8).

use bevy::prelude::*;

use crate::debug::{ClientFrameIndex, CommandTraceBuffer, PendingSimulationTrace};
use crate::simulation::{SimulationControlState, SIMULATION_TICK_SECONDS};
use crate::ui::gameplay::PlayerHudState;
use crate::units::input::SelectedUnits;
use crate::world::{
    step_all_unit_movement, AttackTargetingPolicy, DoodadCatalog, NavigationConfig, UnitCatalog,
    WeaponCatalog, WorldData,
};

/// Advance authoritative unit movement each frame when simulation is not paused.
pub fn tick_unit_movement(
    _time: Res<Time>,
    mut control: ResMut<SimulationControlState>,
    mut world: ResMut<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    nav_config: Res<NavigationConfig>,
    mut pending_trace: ResMut<PendingSimulationTrace>,
) {
    if !control.begin_tick() {
        return;
    }

    let tick = control.current_tick;
    let step_report = step_all_unit_movement(
        &mut world,
        &unit_catalog,
        &weapon_catalog,
        &doodad_catalog,
        &nav_config,
        AttackTargetingPolicy::default(),
        SIMULATION_TICK_SECONDS,
        tick,
    );
    control.complete_tick();

    if !step_report.command_resolve.failures.is_empty()
        || !step_report.command_resolve.successes.is_empty()
    {
        pending_trace.resolve = Some(step_report.command_resolve);
    }
    if !step_report.combat.traces.is_empty() {
        pending_trace.combat = Some(step_report.combat);
    }
    if !step_report.combat_strike.traces.is_empty() {
        pending_trace.combat_strike = Some(step_report.combat_strike);
    }
    if !step_report.death.traces.is_empty() || !step_report.death.removed_unit_ids.is_empty() {
        pending_trace.death = Some(step_report.death);
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
        let weapon_catalog = crate::world::WeaponCatalog::default();
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
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            AttackTargetingPolicy::default(),
            1.0 / 60.0,
            0,
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

/// Apply client-side cleanup after units die or are removed.
pub fn apply_death_client_cleanup(
    world: Res<WorldData>,
    mut selection: ResMut<SelectedUnits>,
    mut hud: ResMut<PlayerHudState>,
    pending: Res<PendingSimulationTrace>,
) {
    if pending.death.is_none() {
        return;
    }
    selection.prune_dead(&world);
    selection.prune_missing(&world);
    crate::ui::gameplay::sync_primary_selection(&mut hud, &selection);
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
    if let Some(report) = pending.combat.take() {
        trace.record_combat_engagement(frame_index.0, &report);
    }
    if let Some(report) = pending.combat_strike.take() {
        trace.record_combat_strike(frame_index.0, &report);
    }
    if let Some(report) = pending.death.take() {
        trace.record_unit_death(frame_index.0, &report);
    }
}
