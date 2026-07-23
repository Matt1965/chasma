//! Local unit movement tick for player-issued orders (ADR-030, ADR-033 U8).

use bevy::prelude::*;

use crate::debug::{
    ClientFrameIndex, CommandTraceBuffer, MovementBlockObservability, PendingSimulationTrace,
};
use crate::simulation::{
    BuildingSimulationParams, SIMULATION_TICK_SECONDS, SimulationCatalogParams, SimulationClock,
    SimulationControlState, SimulationTickReport, run_simulation_tick,
};
use crate::ui::gameplay::PlayerHudState;
use crate::units::input::SelectedUnits;
use crate::world::{
    AttackTargetingPolicy, BuildingCatalog, BuildingConstructionSettings, CombatAiReport,
    CombatAiScanState, CombatAiSettings, CombatEngagementReport, CombatStrikeReport,
    CommandBufferResolveReport, DoodadCatalog, FootprintCatalog, NavigationConfig,
    ProjectileReport, UnitCatalog, UnitDeathReport, WeaponCatalog, WorldData,
};

fn merge_step_trace(pending: &mut PendingSimulationTrace, step_report: &SimulationTickReport) {
    if !step_report.command_resolve.failures.is_empty()
        || !step_report.command_resolve.successes.is_empty()
        || step_report.command_resolve.resolved > 0
        || step_report.command_resolve.failed > 0
    {
        let slot = pending
            .resolve
            .get_or_insert_with(CommandBufferResolveReport::default);
        slot.resolved = slot
            .resolved
            .saturating_add(step_report.command_resolve.resolved);
        slot.failed = slot
            .failed
            .saturating_add(step_report.command_resolve.failed);
        slot.failures
            .extend(step_report.command_resolve.failures.iter().cloned());
        slot.successes
            .extend(step_report.command_resolve.successes.iter().copied());
    }
    if !step_report.combat.traces.is_empty() {
        let slot = pending
            .combat
            .get_or_insert_with(CombatEngagementReport::default);
        slot.traces
            .extend(step_report.combat.traces.iter().cloned());
    }
    if !step_report.combat_strike.traces.is_empty() {
        let slot = pending
            .combat_strike
            .get_or_insert_with(CombatStrikeReport::default);
        slot.traces
            .extend(step_report.combat_strike.traces.iter().cloned());
    }
    if !step_report.projectile.traces.is_empty() {
        let slot = pending
            .projectile
            .get_or_insert_with(ProjectileReport::default);
        slot.traces
            .extend(step_report.projectile.traces.iter().cloned());
    }
    if !step_report.death.traces.is_empty() || !step_report.death.removed_unit_ids.is_empty() {
        let slot = pending.death.get_or_insert_with(UnitDeathReport::default);
        slot.traces.extend(step_report.death.traces.iter().cloned());
        slot.removed_unit_ids
            .extend(step_report.death.removed_unit_ids.iter().copied());
    }
    if !step_report.combat_ai.traces.is_empty() {
        let slot = pending
            .combat_ai
            .get_or_insert_with(CombatAiReport::default);
        slot.traces
            .extend(step_report.combat_ai.traces.iter().cloned());
    }
}

/// Advance authoritative simulation using a fixed timestep clock (ADR-064).
pub fn tick_unit_movement(
    time: Res<Time>,
    mut control: ResMut<SimulationControlState>,
    mut clock: ResMut<SimulationClock>,
    mut world: ResMut<WorldData>,
    mut pending_trace: ResMut<PendingSimulationTrace>,
    mut movement_blocks: ResMut<MovementBlockObservability>,
    mut combat_ai_scan: ResMut<CombatAiScanState>,
    catalogs: SimulationCatalogParams,
    mut building_sim: BuildingSimulationParams,
) {
    let plan = clock.plan_frame(time.delta_secs(), &control);
    for _ in 0..plan.tick_count {
        if !control.begin_tick() {
            break;
        }

        let tick = control.current_tick;
        let inventory_ctx = catalogs.inventory_ctx();
        let mut operation = building_sim.operation_params(
            &catalogs.building_catalog,
            &catalogs.footprint_catalog,
            &inventory_ctx,
        );
        let step_report = run_simulation_tick(
            &mut world,
            &catalogs.unit_catalog,
            &catalogs.weapon_catalog,
            &catalogs.doodad_catalog,
            &catalogs.building_catalog,
            &catalogs.footprint_catalog,
            &catalogs.interaction_catalog,
            &catalogs.nav_config,
            AttackTargetingPolicy::default(),
            &catalogs.combat_ai_settings,
            &mut combat_ai_scan,
            BuildingConstructionSettings::default(),
            &catalogs.interior_catalog,
            Some(&catalogs.nav_blueprint_catalog),
            inventory_ctx.items,
            inventory_ctx.categories,
            inventory_ctx.profiles,
            &catalogs.corpse_settings,
            SIMULATION_TICK_SECONDS,
            tick,
            Some(&mut operation),
        );
        control.complete_tick();
        merge_step_trace(&mut pending_trace, &step_report);
        let fresh_blocks = movement_blocks.filter_new_block_traces(&step_report.movement.traces);
        movement_blocks.apply_batch_traces(&step_report.movement.traces);
        pending_trace.movement_traces.extend(fresh_blocks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{
        SIMULATION_TICK_SECONDS, SimulationClock, SimulationControlState, run_simulation_tick,
    };
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout, FootprintCatalog,
        Heightfield, LocalPosition, UnitCatalog, UnitDefinitionId, UnitOrder, UnitOwnership,
        UnitSource, WeaponCatalog, WorldData, WorldPosition, create_unit,
        create_unit_with_ownership, issue_unit_order, resolve_all_pending_unit_orders,
        starter_unit_definitions, starter_weapon_definitions,
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

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn run_frames(
        clock: &mut SimulationClock,
        control: &mut SimulationControlState,
        world: &mut WorldData,
        catalog: &UnitCatalog,
        weapon_catalog: &WeaponCatalog,
        doodad_catalog: &DoodadCatalog,
        nav_config: &NavigationConfig,
        combat_ai_settings: &CombatAiSettings,
        combat_ai_scan: &mut CombatAiScanState,
        frame_delta: f32,
        frame_count: u32,
    ) {
        let building_catalog = BuildingCatalog::default();
        let footprint_catalog = FootprintCatalog::default();
        for _ in 0..frame_count {
            let plan = clock.plan_frame(frame_delta, control);
            for _ in 0..plan.tick_count {
                if !control.begin_tick() {
                    break;
                }
                let tick = control.current_tick;
                let _ = run_simulation_tick(
                    world,
                    catalog,
                    weapon_catalog,
                    doodad_catalog,
                    &building_catalog,
                    &footprint_catalog,
                    &crate::world::BuildingInteractionProfileCatalog::default(),
                    nav_config,
                    AttackTargetingPolicy::default(),
                    combat_ai_settings,
                    combat_ai_scan,
                    crate::world::BuildingConstructionSettings::default(),
                    &crate::world::InteriorProfileCatalog::default(),
                    None,
                    &crate::world::ItemCatalog::default(),
                    &crate::world::ItemCategoryCatalog::default(),
                    &crate::world::InventoryProfileCatalog::default(),
                    &crate::world::CorpseSettings::default(),
                    SIMULATION_TICK_SECONDS,
                    tick,
                    None,
                );
                control.complete_tick();
            }
        }
    }

    fn issue_move(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        weapon_catalog: &WeaponCatalog,
        doodad_catalog: &DoodadCatalog,
        nav_config: &NavigationConfig,
        unit_id: crate::world::UnitId,
        target: WorldPosition,
    ) {
        issue_unit_order(
            world,
            catalog,
            weapon_catalog,
            doodad_catalog,
            nav_config,
            unit_id,
            UnitOrder::MoveTo { target },
            AttackTargetingPolicy::default(),
        )
        .unwrap();
        let bundle = crate::world::TestPassabilityBundle::new();
        resolve_all_pending_unit_orders(
            world,
            catalog,
            bundle.catalogs_for(doodad_catalog),
            nav_config,
        );
    }

    #[test]
    fn paused_gate_skips_movement_without_partial_execution() {
        let (mut world, catalog, unit_id) = test_world_with_unit();
        let doodad_catalog = DoodadCatalog::default();
        let weapon_catalog = WeaponCatalog::default();
        let nav_config = NavigationConfig::default();
        let before = world.get_unit(unit_id).unwrap().placement.position;

        let mut control = SimulationControlState {
            paused: true,
            ..Default::default()
        };
        let mut clock = SimulationClock::default();
        run_frames(
            &mut clock,
            &mut control,
            &mut world,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &CombatAiSettings::default(),
            &mut CombatAiScanState::default(),
            1.0 / 60.0,
            60,
        );

        let after = world.get_unit(unit_id).unwrap().placement.position;
        assert_eq!(before, after);
        assert_eq!(control.current_tick, 0);
    }

    #[test]
    fn movement_position_matches_across_thirty_and_sixty_fps() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let target = pos(100.0, 0.0);

        let mut world_a = flat_world();
        let unit_a = create_unit(
            &catalog,
            &mut world_a,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        issue_move(
            &mut world_a,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            unit_a,
            target,
        );

        let mut world_b = flat_world();
        let unit_b = create_unit(
            &catalog,
            &mut world_b,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        issue_move(
            &mut world_b,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            unit_b,
            target,
        );

        let settings = CombatAiSettings::default();
        let mut scan_a = CombatAiScanState::default();
        let mut scan_b = CombatAiScanState::default();
        let mut clock_a = SimulationClock::default();
        let mut clock_b = SimulationClock::default();
        let mut control_a = SimulationControlState::default();
        let mut control_b = SimulationControlState::default();

        run_frames(
            &mut clock_a,
            &mut control_a,
            &mut world_a,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_a,
            1.0 / 30.0,
            30,
        );
        run_frames(
            &mut clock_b,
            &mut control_b,
            &mut world_b,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_b,
            1.0 / 60.0,
            60,
        );

        let pos_a = world_a.get_unit(unit_a).unwrap().placement.position;
        let pos_b = world_b.get_unit(unit_b).unwrap().placement.position;
        assert_eq!(control_a.current_tick, control_b.current_tick);
        assert_eq!(pos_a, pos_b);
    }

    #[test]
    fn combat_timer_matches_across_thirty_and_one_twenty_fps() {
        use crate::world::{AttackCycle, CombatState};

        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();

        let mut setup = |world: &mut WorldData| -> (crate::world::UnitId, crate::world::UnitId) {
            let player = create_unit_with_ownership(
                &catalog,
                world,
                &UnitDefinitionId::new("wolf"),
                pos(10.0, 10.0),
                UnitSource::Authored,
                UnitOwnership::player_default(),
            )
            .unwrap()
            .id;
            let hostile = create_unit_with_ownership(
                &catalog,
                world,
                &UnitDefinitionId::new("wolf"),
                pos(11.0, 10.0),
                UnitSource::Authored,
                UnitOwnership::hostile(),
            )
            .unwrap()
            .id;
            world
                .set_unit_combat_state(player, CombatState::Attacking { target: hostile })
                .unwrap();
            world
                .set_unit_attack_cycle(player, Some(AttackCycle::start_windup(hostile, 1.0)))
                .unwrap();
            (player, hostile)
        };

        let mut world_a = flat_world();
        let (player_a, _) = setup(&mut world_a);
        let mut world_b = flat_world();
        let (player_b, _) = setup(&mut world_b);

        let settings = CombatAiSettings::default();
        let mut scan_a = CombatAiScanState::default();
        let mut scan_b = CombatAiScanState::default();
        let mut clock_a = SimulationClock::default();
        let mut clock_b = SimulationClock::default();
        let mut control_a = SimulationControlState::default();
        let mut control_b = SimulationControlState::default();

        run_frames(
            &mut clock_a,
            &mut control_a,
            &mut world_a,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_a,
            1.0 / 30.0,
            5,
        );
        run_frames(
            &mut clock_b,
            &mut control_b,
            &mut world_b,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_b,
            1.0 / 120.0,
            20,
        );

        let cycle_a = world_a
            .get_unit(player_a)
            .unwrap()
            .attack_cycle
            .clone()
            .expect("attack cycle should remain active");
        let cycle_b = world_b
            .get_unit(player_b)
            .unwrap()
            .attack_cycle
            .clone()
            .expect("attack cycle should remain active");
        assert_eq!(control_a.current_tick, control_b.current_tick);
        assert_eq!(cycle_a, cycle_b);
    }

    #[test]
    fn ai_scan_cadence_matches_across_render_rates() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let settings = CombatAiSettings::default();

        let mut world_a = flat_world();
        let _ = create_unit_with_ownership(
            &catalog,
            &mut world_a,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap();

        let mut world_b = flat_world();
        let _ = create_unit_with_ownership(
            &catalog,
            &mut world_b,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap();

        let mut scan_a = CombatAiScanState::default();
        let mut scan_b = CombatAiScanState::default();
        let mut clock_a = SimulationClock::default();
        let mut clock_b = SimulationClock::default();
        let mut control_a = SimulationControlState::default();
        let mut control_b = SimulationControlState::default();

        run_frames(
            &mut clock_a,
            &mut control_a,
            &mut world_a,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_a,
            1.0 / 30.0,
            30,
        );
        run_frames(
            &mut clock_b,
            &mut control_b,
            &mut world_b,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_b,
            1.0 / 120.0,
            120,
        );

        assert_eq!(scan_a.seconds_since_scan, scan_b.seconds_since_scan);
        assert_eq!(control_a.current_tick, control_b.current_tick);
    }

    #[test]
    fn step_once_advances_exactly_one_authoritative_tick() {
        let (mut world, catalog, unit_id) = test_world_with_unit();
        let weapon_catalog = WeaponCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let target = pos(50.0, 50.0);
        issue_move(
            &mut world,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            unit_id,
            target,
        );

        let mut control = SimulationControlState {
            paused: true,
            step_once: true,
            ..Default::default()
        };
        let mut clock = SimulationClock::default();
        let before = world.get_unit(unit_id).unwrap().placement.position;
        run_frames(
            &mut clock,
            &mut control,
            &mut world,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &CombatAiSettings::default(),
            &mut CombatAiScanState::default(),
            1.0,
            1,
        );
        let after = world.get_unit(unit_id).unwrap().placement.position;

        assert_eq!(control.current_tick, 1);
        assert!(control.paused);
        assert_ne!(before, after);
    }

    #[test]
    fn projectile_position_matches_across_render_rates() {
        use crate::world::{
            DamageType, ProjectileLaunchSnapshot, ProjectileRecord, WeaponDefinitionId,
        };

        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let weapon_id = WeaponDefinitionId::new("weapon_test_bow");
        let start = pos(10.0, 10.0);
        let target_pos = pos(30.0, 10.0);

        let mut world_a = flat_world();
        let source_a = create_unit(
            &catalog,
            &mut world_a,
            &UnitDefinitionId::new("wolf"),
            start,
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let target_a = create_unit(
            &catalog,
            &mut world_a,
            &UnitDefinitionId::new("wolf"),
            target_pos,
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let mut world_b = flat_world();
        let source_b = create_unit(
            &catalog,
            &mut world_b,
            &UnitDefinitionId::new("wolf"),
            start,
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let target_b = create_unit(
            &catalog,
            &mut world_b,
            &UnitDefinitionId::new("wolf"),
            target_pos,
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let insert_projectile =
            |world: &mut WorldData, source: crate::world::UnitId, target: crate::world::UnitId| {
                let projectile_id = world.allocate_projectile_id();
                world.insert_projectile(ProjectileRecord::new_in_flight(
                    projectile_id,
                    source,
                    target,
                    weapon_id.clone(),
                    1.0,
                    DamageType::Piercing,
                    start,
                    target_pos,
                    30.0,
                    ProjectileLaunchSnapshot::render_test_placeholder(source),
                ));
            };
        insert_projectile(&mut world_a, source_a, target_a);
        insert_projectile(&mut world_b, source_b, target_b);

        let settings = CombatAiSettings::default();
        let mut scan_a = CombatAiScanState::default();
        let mut scan_b = CombatAiScanState::default();
        let mut clock_a = SimulationClock::default();
        let mut clock_b = SimulationClock::default();
        let mut control_a = SimulationControlState::default();
        let mut control_b = SimulationControlState::default();

        run_frames(
            &mut clock_a,
            &mut control_a,
            &mut world_a,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_a,
            1.0 / 30.0,
            5,
        );
        run_frames(
            &mut clock_b,
            &mut control_b,
            &mut world_b,
            &catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            &settings,
            &mut scan_b,
            1.0 / 120.0,
            20,
        );

        let pos_a = world_a
            .projectiles()
            .next()
            .expect("projectile should remain in flight")
            .1
            .position;
        let pos_b = world_b
            .projectiles()
            .next()
            .expect("projectile should remain in flight")
            .1
            .position;
        assert_eq!(control_a.current_tick, control_b.current_tick);
        assert_eq!(pos_a, pos_b);
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
    if let Some(report) = pending.projectile.take() {
        trace.record_projectile(frame_index.0, &report);
    }
    if let Some(report) = pending.death.take() {
        trace.record_unit_death(frame_index.0, &report);
    }
    if let Some(report) = pending.combat_ai.take() {
        trace.record_combat_ai(frame_index.0, &report);
    }
    if !pending.movement_traces.is_empty() {
        trace.record_unit_movement(frame_index.0, &pending.movement_traces);
        pending.movement_traces.clear();
    }
}
