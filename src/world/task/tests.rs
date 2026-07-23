//! B8 task system tests (ADR-085).

use bevy::prelude::*;

use super::{
    TaskCancelReason, TaskPriority, TaskState, TaskType, assign_construct_building_task,
    assign_operate_workstation_task, cancel_unit_task, ensure_building_task,
    sync_construction_tasks,
};
use crate::world::{
    BuildingConstructionSettings, BuildingInteractionProfileCatalog, BuildingLifecycleState,
    BuildingOwnership, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
    FootprintCatalog, InteriorProfileCatalog, LocalPosition, OccupancyCatalogs, UnitCatalog,
    UnitDefinitionId, UnitOwnership, UnitSource, WeaponCatalog, WorldData, WorldPosition,
    create_unit_with_ownership, default_building_catalog, default_footprint_catalog,
    default_passability, place_player_building, prune_invalid_building_tasks,
    step_all_building_construction, step_all_worker_tasks,
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

fn place_hut(world: &mut WorldData, catalogs: OccupancyCatalogs<'_>) -> crate::world::BuildingId {
    place_player_building(
        catalogs.building,
        world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        catalogs,
    )
    .unwrap()
    .id
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

#[test]
fn construction_task_generated_on_sync() {
    let (building, doodad, footprint) = catalogs();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_hut(&mut world, occ);
    sync_construction_tasks(&mut world, &building, 1);
    let tasks = world.task_store().building_task_ids(building_id);
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        world.task_store().get(tasks[0]).unwrap().task_type,
        TaskType::ConstructBuilding
    );
}

#[test]
fn eligible_worker_assigned_and_reserves_point() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = crate::world::NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_hut(&mut world, occ);
    let worker = builder_unit(&mut world, &unit_catalog, pos(60.0, 64.0));
    let (task_id, _) = assign_construct_building_task(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        worker,
        building_id,
        1,
    )
    .expect("assign construct");
    let task = world.task_store().get(task_id).unwrap();
    assert_eq!(task.assigned_unit_id, Some(worker));
    assert!(task.reserved_point_key.is_some());
    assert!(
        world
            .task_store()
            .reservation_for_point(building_id, task.reserved_point_key.as_ref().unwrap())
            .is_some()
    );
}

#[test]
fn duplicate_reservation_uses_distinct_points() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = crate::world::NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_hut(&mut world, occ);
    let worker_a = builder_unit(&mut world, &unit_catalog, pos(60.0, 64.0));
    let worker_b = builder_unit(&mut world, &unit_catalog, pos(61.0, 64.0));
    assign_construct_building_task(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        worker_a,
        building_id,
        1,
    )
    .unwrap();
    let second = assign_construct_building_task(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        worker_b,
        building_id,
        2,
    );
    assert!(second.is_ok());
    let keys: Vec<_> = world
        .task_store()
        .reservations()
        .map(|r| r.point_key)
        .collect();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn worker_labor_advances_construction_deterministically() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let interior = InteriorProfileCatalog::default();
    let nav = crate::world::NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_hut(&mut world, occ);
    let worker = builder_unit(&mut world, &unit_catalog, pos(64.0, 62.5));
    assign_construct_building_task(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        worker,
        building_id,
        1,
    )
    .unwrap();
    let task_id = world.task_store().unit_task_id(worker).unwrap();
    let building_record = world.get_building(building_id).unwrap().clone();
    let profile = interaction
        .profile_for_definition(building.get(&building_record.definition_id).unwrap())
        .unwrap();
    let point_key = world
        .task_store()
        .get(task_id)
        .unwrap()
        .reserved_point_key
        .as_deref()
        .expect("reserved point");
    let point = profile
        .points
        .iter()
        .find(|point| point.key == point_key)
        .expect("point def");
    let work_pos =
        crate::world::interaction_point_world_position(&building_record, world.layout(), point);
    world
        .relocate_unit(worker, work_pos)
        .expect("relocate worker");
    world
        .set_unit_state(worker, crate::world::UnitState::Working { task_id })
        .unwrap();
    world.mutate_building(building_id, |record| {
        record.lifecycle_state = BuildingLifecycleState::InProgress;
    });
    let build_time = building
        .get(&crate::world::BuildingDefinitionId::new("hut"))
        .unwrap()
        .build_time_seconds;
    let delta = 1.0 / 60.0;
    let ticks = (build_time / delta).ceil() as u32 + 5;
    for _ in 0..ticks {
        let _ = step_all_worker_tasks(
            &mut world,
            &unit_catalog,
            &building,
            &interaction,
            &interior,
            &doodad,
            occ,
            delta,
            None,
        );
    }
    let record = world.get_building(building_id).unwrap();
    assert_eq!(record.lifecycle_state, BuildingLifecycleState::Complete);
    assert!(world.task_store().unit_task_id(worker).is_none());
}

#[test]
fn auto_progress_disabled_only_worker_labor_completes() {
    let (building, doodad, footprint) = catalogs();
    let interior = InteriorProfileCatalog::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_hut(&mut world, occ);
    let report = step_all_building_construction(
        &mut world,
        &building,
        &interior,
        &doodad,
        occ,
        None,
        BuildingConstructionSettings::default(),
        1.0,
    );
    assert_eq!(report.advanced, 0);
    assert_eq!(
        world.get_building(building_id).unwrap().lifecycle_state,
        BuildingLifecycleState::Planned
    );
}

#[test]
fn cancellation_releases_reservation() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = crate::world::NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_hut(&mut world, occ);
    let worker = builder_unit(&mut world, &unit_catalog, pos(60.0, 64.0));
    assign_construct_building_task(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        worker,
        building_id,
        1,
    )
    .unwrap();
    let point_key = world
        .task_store()
        .get(world.task_store().unit_task_id(worker).unwrap())
        .unwrap()
        .reserved_point_key
        .clone()
        .unwrap();
    let mut events = Vec::new();
    cancel_unit_task(
        &mut world,
        worker,
        TaskCancelReason::PlayerOrder,
        &mut events,
    );
    assert!(
        world
            .task_store()
            .reservation_for_point(building_id, &point_key)
            .is_none()
    );
}

#[test]
fn building_destruction_prunes_tasks() {
    let (building, doodad, footprint) = catalogs();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let building_id = place_hut(&mut world, occ);
    ensure_building_task(
        &mut world,
        building_id,
        TaskType::ConstructBuilding,
        TaskPriority::Normal,
        1,
    )
    .unwrap();
    crate::world::destroy_building(
        &mut world,
        &building,
        &doodad,
        occ,
        building_id,
        "test",
        None,
    )
    .unwrap();
    prune_invalid_building_tasks(&mut world);
    assert!(world.task_store().building_task_ids(building_id).is_empty());
}

#[test]
fn completed_workbench_accepts_operate_assignment() {
    let (building, doodad, footprint) = catalogs();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let interior = InteriorProfileCatalog::default();
    let nav = crate::world::NavigationConfig::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let workbench_id = place_player_building(
        &building,
        &mut world,
        &crate::world::BuildingDefinitionId::new("workbench"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    world.mutate_building(workbench_id, |record| {
        record.lifecycle_state = BuildingLifecycleState::Complete;
        record.construction.progress_0_1 = 1.0;
        record.vitals.current_hp = record.vitals.max_hp;
    });
    let worker = builder_unit(&mut world, &unit_catalog, pos(80.0, 79.0));
    let result = assign_operate_workstation_task(
        &mut world,
        &unit_catalog,
        &weapons,
        &doodad,
        &building,
        &interaction,
        &nav,
        worker,
        workbench_id,
        1,
    );
    assert!(result.is_ok());
    let task = world.task_store().get(result.unwrap().0).unwrap();
    assert_eq!(task.state, TaskState::Assigned);
    let _ = interior;
}
