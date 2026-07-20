//! Task marketplace / Worker Assignment tests (SA7).

use bevy::prelude::*;

use super::*;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::task::{
    sync_construction_tasks, TaskPriority, TaskState, TaskType,
};
use crate::world::{
    BuildingInteractionProfileCatalog, BuildingOwnership, ChunkCoord, ChunkData, ChunkId,
    ChunkLayout, DoodadCatalog, FootprintCatalog, LocalPosition, NavigationConfig,
    OccupancyCatalogs, PassabilityCatalogs, UnitCatalog, UnitDefinitionId, UnitOwnership,
    UnitSource, UnitState, WeaponCatalog, WorldData, WorldPosition, create_unit_with_ownership,
    place_player_building, resolve_pending_unit_orders,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn catalogs() -> (
    crate::world::BuildingCatalog,
    DoodadCatalog,
    FootprintCatalog,
) {
    (
        crate::world::BuildingCatalog::default(),
        DoodadCatalog::default(),
        FootprintCatalog::default(),
    )
}

fn occ<'a>(
    building: &'a crate::world::BuildingCatalog,
    doodad: &'a DoodadCatalog,
    footprint: &'a FootprintCatalog,
) -> OccupancyCatalogs<'a> {
    OccupancyCatalogs {
        building,
        doodad,
        footprint,
    }
}

fn empty_inventory_ctx() -> InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<(
        crate::world::ItemCatalog,
        crate::world::ItemCategoryCatalog,
        crate::world::InventoryProfileCatalog,
    )> = std::sync::OnceLock::new();
    let (items, categories, profiles) = CTX.get_or_init(|| {
        let categories = crate::world::ItemCategoryCatalog::from_definitions(
            crate::world::starter_item_category_definitions(),
        )
        .unwrap();
        let items = crate::world::ItemCatalog::from_definitions(
            crate::world::starter_item_definitions(),
            &categories,
        )
        .unwrap();
        let profiles = crate::world::InventoryProfileCatalog::from_definitions(
            crate::world::starter_inventory_profile_definitions(),
        )
        .unwrap();
        (items, categories, profiles)
    });
    InventoryCatalogCtx::new(items, categories, profiles)
}

fn builder_unit(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    at: WorldPosition,
) -> crate::world::UnitId {
    create_unit_with_ownership(
        catalog,
        world,
        &UnitDefinitionId::new("bandit"),
        at,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id
}

fn run_assign(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapons: &WeaponCatalog,
    doodad: &DoodadCatalog,
    building: &crate::world::BuildingCatalog,
    interaction: &BuildingInteractionProfileCatalog,
    nav: &NavigationConfig,
    tick: u64,
) -> WorkerAssignmentReport {
    let inventory_ctx = empty_inventory_ctx();
    let mut ctx = WorkerAssignmentContext {
        world,
        unit_catalog,
        weapon_catalog: weapons,
        doodad_catalog: doodad,
        building_catalog: building,
        interaction_catalog: interaction,
        nav_config: nav,
        inventory_ctx: &inventory_ctx,
        simulation_tick: tick,
    };
    step_worker_assignment(&mut ctx)
}

#[test]
fn idle_worker_autonomously_acquires_construction_task() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    sync_construction_tasks(&mut world, &building, 1);
    let worker = builder_unit(&mut world, &unit_catalog, pos(60.0, 64.0));

    let report = run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        2,
    );
    assert!(
        !report.assignments.is_empty(),
        "expected autonomous assign; diag={:?}",
        report.diagnostics
    );
    assert_eq!(world.task_store().unit_task_id(worker), report.assignments[0].task_id);
    let task_id = report.assignments[0].task_id.unwrap();
    let task = world.task_store().get(task_id).unwrap();
    assert_eq!(task.task_type, TaskType::ConstructBuilding);
    assert!(task.reserved_point_key.is_some());
    assert!(matches!(
        task.state,
        TaskState::Assigned | TaskState::InProgress
    ));
}

#[test]
fn reservations_prevent_duplicate_point_claim() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    sync_construction_tasks(&mut world, &building, 1);
    let _a = builder_unit(&mut world, &unit_catalog, pos(60.0, 64.0));
    let _b = builder_unit(&mut world, &unit_catalog, pos(61.0, 64.0));

    run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        2,
    );

    let keys: Vec<_> = world
        .task_store()
        .reservations()
        .filter(|r| r.building_id == building_id)
        .map(|r| r.point_key)
        .collect();
    assert!(keys.len() >= 1);
    let unique: std::collections::BTreeSet<_> = keys.iter().cloned().collect();
    assert_eq!(keys.len(), unique.len(), "duplicate reservation points: {keys:?}");
}

#[test]
fn higher_priority_listing_wins() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let low_id = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(80.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    let high_id = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(50.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    sync_construction_tasks(&mut world, &building, 1);
    // Force priorities after sync.
    for task_id in world.task_store().sorted_task_ids() {
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            if task.target_building_id() == low_id {
                task.priority = TaskPriority::Low;
            }
            if task.target_building_id() == high_id {
                task.priority = TaskPriority::High;
            }
        }
    }
    let worker = builder_unit(&mut world, &unit_catalog, pos(65.0, 64.0));
    let report = run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        2,
    );
    let task_id = report.assignments[0].task_id.unwrap();
    let task = world.task_store().get(task_id).unwrap();
    assert_eq!(task.target_building_id(), high_id);
    assert_eq!(task.priority, TaskPriority::High);
    assert_eq!(world.task_store().unit_task_id(worker), Some(task_id));
}

#[test]
fn workers_resume_idle_assignment_after_load_seam() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let _building_id = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    sync_construction_tasks(&mut world, &building, 1);
    let worker = builder_unit(&mut world, &unit_catalog, pos(60.0, 64.0));
    run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        2,
    );
    let task_id = world.task_store().unit_task_id(worker).expect("assigned");
    // Simulate save/load: unit Idle but still mapped to task + reservation.
    world.set_unit_state(worker, UnitState::Idle).unwrap();
    world.worker_assignment_store_mut().clear();

    let report = run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        3,
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.contains("resumed Idle")),
        "expected resume diag; got {:?}",
        report.diagnostics
    );
    assert_eq!(world.task_store().unit_task_id(worker), Some(task_id));
    // Work orders enqueue; resolve path like the simulation tick does.
    let passability = PassabilityCatalogs {
        doodad: &doodad,
        building: &building,
        footprint: &footprint,
    };
    resolve_pending_unit_orders(&mut world, &unit_catalog, passability, &nav);
    assert!(!matches!(
        world.get_unit(worker).unwrap().state,
        UnitState::Idle
    ));
}

#[test]
fn interrupted_low_priority_recovers_as_available() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let low_id = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(80.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    let high_id = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(50.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    sync_construction_tasks(&mut world, &building, 1);
    let low_task = world
        .task_store()
        .building_task_ids(low_id)
        .first()
        .copied()
        .unwrap();
    let high_task = world
        .task_store()
        .building_task_ids(high_id)
        .first()
        .copied()
        .unwrap();
    world.task_store_mut().get_mut(low_task).unwrap().priority = TaskPriority::Low;
    world.task_store_mut().get_mut(high_task).unwrap().priority = TaskPriority::High;

    // Hide high task first so worker takes low.
    world.task_store_mut().get_mut(high_task).unwrap().state = TaskState::Canceled;
    let worker = builder_unit(&mut world, &unit_catalog, pos(65.0, 64.0));
    run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        2,
    );
    assert_eq!(world.task_store().unit_task_id(worker), Some(low_task));

    // Restore high task and force stick clock so preemption is allowed.
    world.task_store_mut().get_mut(high_task).unwrap().state = TaskState::Available;
    world.worker_assignment_store_mut().note_assignment(worker, 0, false);

    let report = run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        100,
    );
    assert!(
        report.assignments.iter().any(|a| a.preempted),
        "expected preemption; assignments={:?}",
        report.assignments
    );
    assert_eq!(world.task_store().unit_task_id(worker), Some(high_task));
    let low = world.task_store().get(low_task).unwrap();
    assert_eq!(low.state, TaskState::Available);
    assert!(low.assigned_unit_id.is_none());
}

#[test]
fn validation_detects_dead_worker_holding_task() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let _ = place_player_building(
        occ.building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    );
    sync_construction_tasks(&mut world, &building, 1);
    let worker = builder_unit(&mut world, &unit_catalog, pos(60.0, 64.0));
    run_assign(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        2,
    );
    world.set_unit_state(worker, UnitState::Dead).unwrap();
    // Bypass release_dead by validating before step.
    let errors = validate_worker_assignments(&world, &unit_catalog);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, AssignmentValidationError::DeadWorkerHoldingTask { .. })),
        "errors={errors:?}"
    );
}
