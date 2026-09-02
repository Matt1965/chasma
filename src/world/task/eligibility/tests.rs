//! Physical capability and settlement membership eligibility tests (Phase 3).

use bevy::prelude::{Quat, Vec3};

use super::{unit_can_perform_task, unit_may_autonomously_work_building, unit_work_capabilities};
use crate::world::task::TaskType;
use crate::world::{
    Affiliation, BuildingId, BuildingOwnership, ChunkCoord, ChunkData, ChunkLayout, FactionId,
    LocalPosition, SettlementKind, SettlementOwnership, SpeciesId, UnitCatalog, UnitDefinition,
    UnitDefinitionId, UnitOwnership, UnitRenderKey, UnitSource, UnitWorkCapabilities,
    WeaponDefinitionId, WorldData, WorldPosition, assign_building_settlement,
    assign_construct_building_task, assign_unit_settlement, create_settlement,
    create_unit_with_ownership, place_player_building, starter_unit_definitions,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
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

fn custom_catalog(caps: UnitWorkCapabilities) -> UnitCatalog {
    let definition = UnitDefinition::new(
        UnitDefinitionId::new("test_worker"),
        "Test Worker",
        FactionId::new("player"),
        SpeciesId::new("human"),
        "Player",
        1,
        10,
        10,
        5,
        5,
        5,
        5,
        5,
        5,
        10.0,
        "Common",
        4.0,
        0.5,
        35.0,
        WeaponDefinitionId::new("weapon_fists"),
        true,
        UnitRenderKey::reserved("bandit"),
    )
    .with_work_capabilities(caps);
    UnitCatalog::from_definitions(vec![definition]).unwrap()
}

fn spawn_worker(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    at: WorldPosition,
) -> crate::world::UnitId {
    create_unit_with_ownership(
        catalog,
        world,
        &UnitDefinitionId::new("test_worker"),
        at,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id
}

fn settlement_with_building(world: &mut WorldData) -> (crate::world::SettlementId, BuildingId) {
    let settlement = create_settlement(
        world,
        pos(64.0, 64.0),
        "Town",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    let building = place_player_building(
        &crate::world::BuildingCatalog::default(),
        world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        crate::world::OccupancyCatalogs {
            building: &crate::world::BuildingCatalog::default(),
            doodad: &crate::world::DoodadCatalog::default(),
            footprint: &crate::world::FootprintCatalog::default(),
        },
    )
    .unwrap();
    assign_building_settlement(world, building.id, Some(settlement.settlement_id)).unwrap();
    (settlement.settlement_id, building.id)
}

#[test]
fn haul_uses_can_haul_not_operate_workstation() {
    let catalog = custom_catalog(UnitWorkCapabilities {
        can_construct: false,
        construction_speed: 1.0,
        can_operate_workstation: false,
        can_haul: true,
    });
    let mut world = flat_world();
    let unit_id = spawn_worker(&mut world, &catalog, pos(1.0, 1.0));
    assert!(unit_can_perform_task(
        &catalog,
        &world,
        unit_id,
        TaskType::Haul
    ));
    assert!(!unit_can_perform_task(
        &catalog,
        &world,
        unit_id,
        TaskType::OperateWorkstation
    ));
}

#[test]
fn haul_false_blocks_even_when_operate_true() {
    let world = flat_world();
    let catalog = custom_catalog(UnitWorkCapabilities {
        can_construct: false,
        construction_speed: 1.0,
        can_operate_workstation: true,
        can_haul: false,
    });
    let mut world = world;
    let unit_id = spawn_worker(&mut world, &catalog, pos(1.0, 1.0));
    assert!(!unit_can_perform_task(
        &catalog,
        &world,
        unit_id,
        TaskType::Haul
    ));
    assert!(unit_can_perform_task(
        &catalog,
        &world,
        unit_id,
        TaskType::OperateWorkstation
    ));
}

#[test]
fn construct_and_operate_require_matching_physical_flags() {
    let catalog = custom_catalog(UnitWorkCapabilities {
        can_construct: true,
        construction_speed: 1.0,
        can_operate_workstation: true,
        can_haul: true,
    });
    let mut world = flat_world();
    let unit_id = spawn_worker(&mut world, &catalog, pos(1.0, 1.0));
    assert!(unit_can_perform_task(
        &catalog,
        &world,
        unit_id,
        TaskType::ConstructBuilding
    ));
    assert!(unit_can_perform_task(
        &catalog,
        &world,
        unit_id,
        TaskType::OperateWorkstation
    ));
}

#[test]
fn physical_capability_false_blocks_matching_task() {
    let catalog = custom_catalog(UnitWorkCapabilities::default());
    let mut world = flat_world();
    let unit_id = spawn_worker(&mut world, &catalog, pos(1.0, 1.0));
    assert!(!unit_can_perform_task(
        &catalog,
        &world,
        unit_id,
        TaskType::ConstructBuilding
    ));
}

#[test]
fn construction_speed_does_not_affect_binary_eligibility() {
    let slow = custom_catalog(UnitWorkCapabilities {
        can_construct: true,
        construction_speed: 0.1,
        can_operate_workstation: false,
        can_haul: false,
    });
    let fast = custom_catalog(UnitWorkCapabilities {
        can_construct: true,
        construction_speed: 5.0,
        can_operate_workstation: false,
        can_haul: false,
    });
    let mut world = flat_world();
    let slow_id = spawn_worker(&mut world, &slow, pos(1.0, 1.0));
    let fast_id = spawn_worker(&mut world, &fast, pos(2.0, 2.0));
    assert!(unit_can_perform_task(
        &slow,
        &world,
        slow_id,
        TaskType::ConstructBuilding
    ));
    assert!(unit_can_perform_task(
        &fast,
        &world,
        fast_id,
        TaskType::ConstructBuilding
    ));
    let slow_caps = unit_work_capabilities(&slow, &world, slow_id).unwrap();
    let fast_caps = unit_work_capabilities(&fast, &world, fast_id).unwrap();
    assert!(slow_caps.construction_speed < fast_caps.construction_speed);
}

#[test]
fn strategic_task_kinds_are_not_physically_performable() {
    let catalog = custom_catalog(UnitWorkCapabilities::settler_default());
    let mut world = flat_world();
    let unit_id = spawn_worker(&mut world, &catalog, pos(1.0, 1.0));
    for task_type in [
        TaskType::StrategicConstruct,
        TaskType::RepairBuilding,
        TaskType::ClearRubble,
        TaskType::RecruitWorker,
        TaskType::ExpandStorage,
    ] {
        assert!(
            !unit_can_perform_task(&catalog, &world, unit_id, task_type),
            "expected {task_type:?} to remain unavailable"
        );
    }
}

#[test]
fn settlement_membership_required_for_autonomous_building_work() {
    let mut world = flat_world();
    let (settlement_id, building_id) = settlement_with_building(&mut world);
    let catalog = custom_catalog(UnitWorkCapabilities::settler_default());
    let worker = spawn_worker(&mut world, &catalog, pos(60.0, 64.0));
    assert!(!unit_may_autonomously_work_building(
        &world,
        worker,
        building_id
    ));
    assign_unit_settlement(&mut world, worker, Some(settlement_id)).unwrap();
    assert!(unit_may_autonomously_work_building(
        &world,
        worker,
        building_id
    ));
}

#[test]
fn starter_bandit_fixture_has_settler_work_capabilities() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let definition = catalog.get(&UnitDefinitionId::new("bandit")).unwrap();
    assert!(definition.work_capabilities.can_construct);
    assert!(definition.work_capabilities.can_operate_workstation);
    assert!(definition.work_capabilities.can_haul);
}

#[test]
fn player_assigned_construct_bypasses_settlement_membership_gate() {
    let mut world = flat_world();
    let (settlement_a, building_a) = settlement_with_building(&mut world);
    let settlement_b = create_settlement(
        &mut world,
        pos(160.0, 160.0),
        "Far",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        1,
    )
    .unwrap()
    .settlement_id;
    let catalog = custom_catalog(UnitWorkCapabilities::settler_default());
    let outsider = spawn_worker(&mut world, &catalog, pos(62.0, 64.0));
    assign_unit_settlement(&mut world, outsider, Some(settlement_b)).unwrap();
    assert!(!unit_may_autonomously_work_building(
        &world, outsider, building_a
    ));

    let result = assign_construct_building_task(
        &mut world,
        &catalog,
        &crate::world::WeaponCatalog::default(),
        &crate::world::DoodadCatalog::default(),
        &crate::world::BuildingCatalog::default(),
        &crate::world::BuildingInteractionProfileCatalog::default(),
        &crate::world::NavigationConfig::default(),
        outsider,
        building_a,
        2,
    );
    assert!(
        result.is_ok(),
        "player orders should bypass membership gate"
    );
    let _ = settlement_a;
}
