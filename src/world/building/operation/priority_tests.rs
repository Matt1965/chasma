//! Building work priority authority tests.

use bevy::prelude::{Quat, Vec3};

use super::priority::{
    BUILDING_WORK_PRIORITY_HIGH_U8, BUILDING_WORK_PRIORITY_LOW_U8, BuildingWorkPriorityLevel,
    DEFAULT_BUILDING_WORK_PRIORITY_U8, building_work_priority_level_from_u8,
    building_work_priority_to_task_priority, building_work_priority_u8,
    building_work_priority_u8_for_level, step_building_work_priority_level,
};
use super::{
    BuildingOperationPolicy, ControlSource, apply_player_building_work_priority,
    set_building_work_priority,
};
use crate::world::task::{
    TaskPriority, TaskType, WorkerAssignmentContext, step_worker_assignment,
    sync_construction_tasks,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinitionId,
    BuildingInteractionProfileCatalog, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData,
    ChunkLayout, DoodadCatalog, FootprintCatalog, Heightfield, InventoryCatalogCtx, LocalPosition,
    NavigationConfig, OccupancyCatalogs, OperationCatalog, UnitCatalog, UnitDefinitionId,
    UnitOwnership, UnitSource, WeaponCatalog, WorkPermissionDomain, WorkSkillCatalog, WorkSkillId,
    WorldData, WorldPosition, assign_unit_settlement, create_settlement,
    create_unit_with_ownership, place_player_building, set_unit_work_permission,
    starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_operation_definitions,
    starter_work_skill_definitions, work_skill_value,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
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

fn catalogs() -> (
    BuildingCatalog,
    DoodadCatalog,
    FootprintCatalog,
    OperationCatalog,
) {
    let categories = BuildingCategoryCatalog::default();
    let building =
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
    let ops = OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    (
        building,
        DoodadCatalog::default(),
        FootprintCatalog::default(),
        ops,
    )
}

fn inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories = crate::world::ItemCategoryCatalog::from_definitions(
            starter_item_category_definitions(),
        )
        .unwrap();
        let items =
            crate::world::ItemCatalog::from_definitions(starter_item_definitions(), &categories)
                .unwrap();
        let profiles = crate::world::InventoryProfileCatalog::from_definitions(
            starter_inventory_profile_definitions(),
        )
        .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn occ<'a>(
    building: &'a BuildingCatalog,
    doodad: &'a DoodadCatalog,
    footprint: &'a FootprintCatalog,
) -> OccupancyCatalogs<'a> {
    OccupancyCatalogs {
        building,
        doodad,
        footprint,
    }
}

fn run_assign(world: &mut WorldData, building: &BuildingCatalog, tick: u64) {
    let unit_catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let doodad = DoodadCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let operation_catalog = OperationCatalog::default();
    let inventory_ctx = inventory_ctx();
    let mut ctx = WorkerAssignmentContext {
        world,
        unit_catalog: &unit_catalog,
        weapon_catalog: &weapons,
        doodad_catalog: &doodad,
        building_catalog: building,
        operation_catalog: &operation_catalog,
        interaction_catalog: &interaction,
        nav_config: &nav,
        inventory_ctx: &inventory_ctx,
        simulation_tick: tick,
    };
    step_worker_assignment(&mut ctx);
}

#[test]
fn new_building_defaults_to_neutral_priority() {
    let policy = BuildingOperationPolicy::default();
    assert_eq!(policy.priority, DEFAULT_BUILDING_WORK_PRIORITY_U8);
    assert_eq!(
        building_work_priority_level_from_u8(policy.priority),
        BuildingWorkPriorityLevel::Normal
    );
    assert_eq!(
        building_work_priority_to_task_priority(policy.priority),
        TaskPriority::Normal
    );
}

#[test]
fn player_can_change_priority_through_authoritative_api() {
    let mut world = flat_world();
    let (building, _, _, ops) = catalogs();
    let hut = place_player_building(
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        )
        .building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        ),
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    assert_eq!(
        world
            .building_production_store()
            .get_policy(hut)
            .unwrap()
            .priority,
        BUILDING_WORK_PRIORITY_HIGH_U8
    );
}

#[test]
fn apply_player_priority_sets_player_controlled() {
    let mut world = flat_world();
    let (building, _, _, ops) = catalogs();
    let hut = place_player_building(
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        )
        .building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        ),
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::Normal,
    )
    .unwrap();
    world
        .building_production_store_mut()
        .get_policy_mut(hut)
        .control_source = ControlSource::AIControlled;
    apply_player_building_work_priority(&mut world, &building, &ops, hut, true).unwrap();
    let policy = world.building_production_store().get_policy(hut).unwrap();
    assert_eq!(policy.priority, BUILDING_WORK_PRIORITY_HIGH_U8);
    assert_eq!(policy.control_source, ControlSource::PlayerControlled);
}

#[test]
fn construction_sync_uses_building_priority() {
    let mut world = flat_world();
    let (building, _, _, ops) = catalogs();
    let hut = place_player_building(
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        )
        .building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        ),
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    let task_ids: Vec<_> = world
        .task_store()
        .building_task_ids(hut)
        .iter()
        .copied()
        .collect();
    let task_id = task_ids
        .into_iter()
        .find(|id| {
            world
                .task_store()
                .get(*id)
                .is_some_and(|task| task.task_type == TaskType::ConstructBuilding)
        })
        .expect("construction task");
    assert_eq!(
        world.task_store().get(task_id).unwrap().priority,
        TaskPriority::High
    );
}

#[test]
fn high_priority_construction_wins_over_normal() {
    let mut world = flat_world();
    let (building, doodad, footprint, ops) = catalogs();
    let occ = occ(&building, &doodad, &footprint);
    let low = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    let high = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        low,
        BuildingWorkPriorityLevel::Low,
    )
    .unwrap();
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        high,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    let unit_catalog = UnitCatalog::default();
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(65.0, 64.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    run_assign(&mut world, &building, 2);
    let task_id = world.task_store().unit_task_id(worker).expect("assigned");
    assert_eq!(
        world
            .task_store()
            .get(task_id)
            .unwrap()
            .target_building_id(),
        high
    );
}

#[test]
fn workforce_permission_still_blocks_high_priority_building() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let (building, doodad, footprint, ops) = catalogs();
    let occ = occ(&building, &doodad, &footprint);
    let settlement_id = create_settlement(
        &mut world,
        pos(64.0, 64.0),
        "Town",
        crate::world::SettlementOwnership::player_default(),
        crate::world::SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(65.0, 64.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    assign_unit_settlement(&mut world, worker, Some(settlement_id)).unwrap();
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Construction,
        false,
    )
    .unwrap();
    let hut = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    crate::world::assign_building_settlement(&mut world, hut, Some(settlement_id)).unwrap();
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    run_assign(&mut world, &building, 2);
    assert!(world.task_store().unit_task_id(worker).is_none());
}

#[test]
fn priority_stepping_is_bounded() {
    assert_eq!(
        step_building_work_priority_level(BuildingWorkPriorityLevel::Low, false),
        BuildingWorkPriorityLevel::Low
    );
    assert_eq!(
        step_building_work_priority_level(BuildingWorkPriorityLevel::High, true),
        BuildingWorkPriorityLevel::High
    );
    assert_eq!(
        building_work_priority_u8_for_level(BuildingWorkPriorityLevel::Low),
        BUILDING_WORK_PRIORITY_LOW_U8
    );
}

#[test]
fn operate_sync_refreshes_available_task_priority() {
    let mut world = flat_world();
    let (building, _, _, ops) = catalogs();
    let hut = place_player_building(
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        )
        .building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        ),
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::Low,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    let task_ids: Vec<_> = world
        .task_store()
        .building_task_ids(hut)
        .iter()
        .copied()
        .collect();
    let task_id = task_ids
        .into_iter()
        .find(|id| {
            world
                .task_store()
                .get(*id)
                .is_some_and(|task| task.task_type == TaskType::ConstructBuilding)
        })
        .expect("construction task");
    assert_eq!(
        world.task_store().get(task_id).unwrap().priority,
        TaskPriority::Low
    );
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 2);
    assert_eq!(
        world.task_store().get(task_id).unwrap().priority,
        TaskPriority::High
    );
}

#[test]
fn low_priority_construction_loses_to_normal() {
    let mut world = flat_world();
    let (building, doodad, footprint, ops) = catalogs();
    let occ = occ(&building, &doodad, &footprint);
    let low = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    let normal = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        low,
        BuildingWorkPriorityLevel::Low,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    let unit_catalog = UnitCatalog::default();
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(65.0, 64.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    run_assign(&mut world, &building, 2);
    let task_id = world.task_store().unit_task_id(worker).expect("assigned");
    assert_eq!(
        world
            .task_store()
            .get(task_id)
            .unwrap()
            .target_building_id(),
        normal
    );
}

#[test]
fn physical_capability_still_blocks_high_priority_building() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let (building, doodad, footprint, ops) = catalogs();
    let occ = occ(&building, &doodad, &footprint);
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(65.0, 64.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let hut = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    run_assign(&mut world, &building, 2);
    assert!(world.task_store().unit_task_id(worker).is_none());
}

#[test]
fn priority_change_does_not_alter_work_skills() {
    let mut world = flat_world();
    let (building, _, _, ops) = catalogs();
    let hut = place_player_building(
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        )
        .building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        ),
    )
    .unwrap()
    .id;
    let unit_catalog = UnitCatalog::default();
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(65.0, 64.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let skill_catalog =
        WorkSkillCatalog::from_definitions(starter_work_skill_definitions()).unwrap();
    let before = work_skill_value(
        &world,
        &skill_catalog,
        worker,
        &WorkSkillId::new("construction"),
    )
    .unwrap();
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    let after = work_skill_value(
        &world,
        &skill_catalog,
        worker,
        &WorkSkillId::new("construction"),
    )
    .unwrap();
    assert_eq!(before, after);
}

#[test]
fn priority_change_does_not_alter_production_policy_enabled() {
    let mut world = flat_world();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
    let operation_catalog =
        OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
    let farm_def = building_catalog
        .get(&BuildingDefinitionId::new("prispod_farm"))
        .unwrap();
    let farm = crate::world::create_building_with_inventory(
        &building_catalog,
        &mut world,
        &farm_def.id,
        pos(10.0, 10.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        inventory_ctx(),
    )
    .unwrap()
    .id;
    {
        let store = world.building_production_store_mut();
        store.ensure_policy_for_building(farm, farm_def, &operation_catalog);
        store.get_policy_mut(farm).enabled = true;
    }
    let enabled_before = world
        .building_production_store()
        .get_policy(farm)
        .unwrap()
        .enabled;
    set_building_work_priority(
        &mut world,
        &building_catalog,
        &operation_catalog,
        farm,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    let enabled_after = world
        .building_production_store()
        .get_policy(farm)
        .unwrap()
        .enabled;
    assert_eq!(enabled_before, enabled_after);
}

#[test]
fn active_worker_not_released_when_other_building_priority_changes() {
    let mut world = flat_world();
    let (building, doodad, footprint, ops) = catalogs();
    let occ = occ(&building, &doodad, &footprint);
    let low = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    let high = place_player_building(
        occ.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        low,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        high,
        BuildingWorkPriorityLevel::Low,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    let high_task = world
        .task_store()
        .building_task_ids(high)
        .first()
        .copied()
        .unwrap();
    world.task_store_mut().get_mut(high_task).unwrap().state =
        crate::world::task::TaskState::Canceled;
    let unit_catalog = UnitCatalog::default();
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(65.0, 64.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    run_assign(&mut world, &building, 2);
    let assigned_low = world
        .task_store()
        .unit_task_id(worker)
        .expect("assigned to low");
    assert_eq!(
        world
            .task_store()
            .get(assigned_low)
            .unwrap()
            .target_building_id(),
        low
    );
    world.task_store_mut().get_mut(high_task).unwrap().state =
        crate::world::task::TaskState::Available;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        high,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    sync_construction_tasks(&mut world, &building, 3);
    run_assign(&mut world, &building, 5);
    assert_eq!(world.task_store().unit_task_id(worker), Some(assigned_low));
}

#[test]
fn building_priority_survives_unrelated_world_reads() {
    let mut world = flat_world();
    let (building, _, _, ops) = catalogs();
    let hut = place_player_building(
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        )
        .building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ(
            &building,
            &DoodadCatalog::default(),
            &FootprintCatalog::default(),
        ),
    )
    .unwrap()
    .id;
    set_building_work_priority(
        &mut world,
        &building,
        &ops,
        hut,
        BuildingWorkPriorityLevel::High,
    )
    .unwrap();
    let _ = world.get_building(hut);
    let _ = world.task_store().building_task_ids(hut);
    assert_eq!(
        building_work_priority_u8(&world, hut),
        BUILDING_WORK_PRIORITY_HIGH_U8
    );
}

#[test]
fn default_priority_preserves_neutral_marketplace_mapping() {
    let policy = BuildingOperationPolicy::default();
    assert_eq!(
        building_work_priority_to_task_priority(policy.priority),
        TaskPriority::Normal
    );
    assert_eq!(policy.priority, DEFAULT_BUILDING_WORK_PRIORITY_U8);
}
