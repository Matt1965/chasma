//! Authoritative simulation tick orchestrator (ADR-065, REVIEW-B1).
//!
//! Coordinates subsystem APIs in the canonical order established in REVIEW-A4.
//! Contains no movement/combat algorithms — only stage sequencing.

use crate::world::BuildingOperationParams;
use crate::world::{
    AttackTargetingPolicy, BuildingCatalog, BuildingConstructionSettings,
    BuildingInteractionProfileCatalog, CombatAiScanState, CombatAiSettings, CombatStrikeReport,
    DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, NavigationConfig, OccupancyCatalogs,
    PassabilityCatalogs, ProjectileReport, UnitCatalog, WeaponCatalog, WorldData,
    prune_invalid_building_tasks, resolve_pending_unit_orders, step_all_building_construction,
    step_all_combat_engagement, step_all_combat_strikes, step_all_projectiles,
    step_all_unit_movement, step_all_worker_tasks, step_combat_ai_acquisition,
    step_unit_death_pipeline, sync_construction_tasks,
};

use super::report::SimulationTickReport;

/// Advance one authoritative simulation tick through all canonical stages (ADR-057 / ADR-065).
///
/// Stage order (REVIEW-A4):
/// 1. resolve_pending_unit_orders
/// 2. step_all_combat_engagement
/// 3. step_all_combat_strikes (may spawn projectiles)
/// 4. step_all_projectiles (skips same-tick spawns)
/// 5. step_unit_death_pipeline
/// 6. step_combat_ai_acquisition
/// 7. step_all_building_construction (auto labor dev-gated)
/// 8. step_all_worker_tasks
/// 9. step_all_unit_movement
pub fn run_simulation_tick(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    combat_ai_settings: &CombatAiSettings,
    combat_ai_scan: &mut CombatAiScanState,
    building_construction_settings: BuildingConstructionSettings,
    interior_catalog: &InteriorProfileCatalog,
    item_catalog: &crate::world::ItemCatalog,
    item_categories: &crate::world::ItemCategoryCatalog,
    inventory_profiles: &crate::world::InventoryProfileCatalog,
    corpse_settings: &crate::world::CorpseSettings,
    delta_seconds: f32,
    simulation_tick: u64,
    mut operation: Option<&mut BuildingOperationParams<'_>>,
) -> SimulationTickReport {
    let passability = PassabilityCatalogs {
        doodad: doodad_catalog,
        building: building_catalog,
        footprint: footprint_catalog,
    };
    let occupancy = OccupancyCatalogs {
        doodad: doodad_catalog,
        building: building_catalog,
        footprint: footprint_catalog,
    };
    let command_resolve = resolve_pending_unit_orders(world, unit_catalog, passability, nav_config);
    let mut combat_strike = CombatStrikeReport::default();
    let combat = step_all_combat_engagement(
        world,
        unit_catalog,
        weapon_catalog,
        passability,
        nav_config,
        targeting_policy,
        &mut combat_strike,
    );
    let mut projectile_spawn = ProjectileReport::default();
    combat_strike = step_all_combat_strikes(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        targeting_policy,
        delta_seconds,
        &mut projectile_spawn,
    );
    let spawned_this_tick = projectile_spawn.spawned_projectile_ids();
    let mut projectile_step = step_all_projectiles(world, delta_seconds, &spawned_this_tick);
    let mut projectile = projectile_spawn;
    projectile.traces.append(&mut projectile_step.traces);
    let inventory_ctx =
        crate::world::InventoryCatalogCtx::new(item_catalog, item_categories, inventory_profiles);
    let death = step_unit_death_pipeline(
        world,
        unit_catalog,
        Some(&inventory_ctx),
        corpse_settings,
        simulation_tick,
    );
    let _corpse_lifecycle = crate::world::step_corpse_lifecycle(world, &inventory_ctx);
    let combat_ai = step_combat_ai_acquisition(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        targeting_policy,
        combat_ai_settings,
        combat_ai_scan,
        delta_seconds,
    );
    let building_construction = step_all_building_construction(
        world,
        building_catalog,
        interior_catalog,
        doodad_catalog,
        occupancy,
        building_construction_settings,
        delta_seconds,
    );
    sync_construction_tasks(world, building_catalog, simulation_tick);
    prune_invalid_building_tasks(world);
    let worker_tasks = step_all_worker_tasks(
        world,
        unit_catalog,
        building_catalog,
        interaction_catalog,
        interior_catalog,
        doodad_catalog,
        occupancy,
        delta_seconds,
        operation.as_deref_mut(),
    );
    let movement = step_all_unit_movement(world, unit_catalog, passability, delta_seconds);
    SimulationTickReport {
        movement,
        command_resolve,
        combat,
        combat_strike,
        projectile,
        death,
        combat_ai,
        building_construction,
        worker_tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::SIMULATION_TICK_SECONDS;
    use crate::world::{
        AttackCycle, AttackPhase, BuildingCatalog, BuildingConstructionSettings, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, CombatState, DoodadCatalog, FootprintCatalog, Heightfield,
        LocalPosition, PassabilityCatalogs, UnitCatalog, UnitDefinitionId, UnitOrder,
        UnitOwnership, UnitSource, WeaponCatalog, WorldPosition, create_unit_with_ownership,
        default_passability, starter_unit_definitions, starter_weapon_definitions,
    };
    use bevy::prelude::Vec3;

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

    fn catalog() -> UnitCatalog {
        UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
    }

    fn weapons() -> WeaponCatalog {
        WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
    }

    #[test]
    fn movement_only_api_does_not_advance_combat_strikes() {
        let catalog = catalog();
        let weapon_catalog = weapons();
        let mut world = flat_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile = create_unit_with_ownership(
            &catalog,
            &mut world,
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
            .set_unit_attack_cycle(player, Some(AttackCycle::start_windup(hostile, 0.5)))
            .unwrap();
        let before = world.get_unit(player).unwrap().attack_cycle.clone();

        let _ = step_all_unit_movement(
            &mut world,
            &catalog,
            default_passability(),
            SIMULATION_TICK_SECONDS,
        );

        assert_eq!(world.get_unit(player).unwrap().attack_cycle, before);
    }

    #[test]
    fn orchestrator_advances_attack_cycle_when_due() {
        let catalog = catalog();
        let weapon_catalog = weapons();
        let mut world = flat_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile = create_unit_with_ownership(
            &catalog,
            &mut world,
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
            .set_unit_attack_cycle(player, Some(AttackCycle::start_windup(hostile, 0.01)))
            .unwrap();

        let mut scan = CombatAiScanState::default();
        let settings = CombatAiSettings::default();
        let report = run_simulation_tick(
            &mut world,
            &catalog,
            &weapon_catalog,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &settings,
            &mut scan,
            BuildingConstructionSettings::default(),
            &InteriorProfileCatalog::default(),
            &crate::world::ItemCatalog::default(),
            &crate::world::ItemCategoryCatalog::default(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::CorpseSettings::default(),
            SIMULATION_TICK_SECONDS,
            1,
            None,
        );

        assert!(!report.combat_strike.traces.is_empty() || report.combat.traces.is_empty());
        let after = world.get_unit(player).unwrap().attack_cycle.clone();
        assert_ne!(after.map(|c| c.phase), Some(AttackPhase::Windup));
    }

    #[test]
    fn orchestrator_aggregates_movement_counts() {
        let catalog = catalog();
        let mut world = flat_world();
        let _ = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        let unit_id = world.sorted_unit_ids()[0];
        crate::world::issue_unit_order(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            unit_id,
            UnitOrder::MoveTo {
                target: pos(5.0, 0.0),
            },
            AttackTargetingPolicy::default(),
        )
        .unwrap();

        let mut scan = CombatAiScanState::default();
        let report = run_simulation_tick(
            &mut world,
            &catalog,
            &weapons(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
            &NavigationConfig::default(),
            AttackTargetingPolicy::default(),
            &CombatAiSettings::default(),
            &mut scan,
            BuildingConstructionSettings::default(),
            &InteriorProfileCatalog::default(),
            &crate::world::ItemCatalog::default(),
            &crate::world::ItemCategoryCatalog::default(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::CorpseSettings::default(),
            SIMULATION_TICK_SECONDS,
            1,
            None,
        );

        assert!(report.orders_resolved() > 0 || report.units_moved() > 0);
        assert_eq!(report.movement.moved, report.units_moved());
    }

    #[test]
    fn repeated_ticks_are_deterministic_for_idle_world() {
        let catalog = catalog();
        let weapon_catalog = weapons();
        let doodads = DoodadCatalog::default();
        let nav = NavigationConfig::default();
        let settings = CombatAiSettings::default();
        let policy = AttackTargetingPolicy::default();

        let mut world_a = flat_world();
        let mut world_b = flat_world();
        let mut scan_a = CombatAiScanState::default();
        let mut scan_b = CombatAiScanState::default();

        for tick in 1..=3 {
            let report_a = run_simulation_tick(
                &mut world_a,
                &catalog,
                &weapon_catalog,
                &doodads,
                &BuildingCatalog::default(),
                &FootprintCatalog::default(),
                &crate::world::BuildingInteractionProfileCatalog::default(),
                &nav,
                policy,
                &settings,
                &mut scan_a,
                BuildingConstructionSettings::default(),
                &InteriorProfileCatalog::default(),
                &crate::world::ItemCatalog::default(),
                &crate::world::ItemCategoryCatalog::default(),
                &crate::world::InventoryProfileCatalog::default(),
                &crate::world::CorpseSettings::default(),
                SIMULATION_TICK_SECONDS,
                tick,
                None,
            );
            let report_b = run_simulation_tick(
                &mut world_b,
                &catalog,
                &weapon_catalog,
                &doodads,
                &BuildingCatalog::default(),
                &FootprintCatalog::default(),
                &crate::world::BuildingInteractionProfileCatalog::default(),
                &nav,
                policy,
                &settings,
                &mut scan_b,
                BuildingConstructionSettings::default(),
                &InteriorProfileCatalog::default(),
                &crate::world::ItemCatalog::default(),
                &crate::world::ItemCategoryCatalog::default(),
                &crate::world::InventoryProfileCatalog::default(),
                &crate::world::CorpseSettings::default(),
                SIMULATION_TICK_SECONDS,
                tick,
                None,
            );
            assert_eq!(report_a, report_b);
        }
        assert_eq!(world_a.sorted_unit_ids(), world_b.sorted_unit_ids());
    }
}
