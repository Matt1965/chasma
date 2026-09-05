//! Workforce permission authority tests.

use bevy::prelude::{Quat, Vec3};

use crate::world::building::operation::is_prispod_farm_definition;
use crate::world::operation::OperationCatalog;
use crate::world::task::{
    TaskType, WorkerAssignmentContext, assign_construct_building_task, step_worker_assignment,
    sync_construction_tasks,
};
use crate::world::{
    Affiliation, BuildingDefinitionId, BuildingInteractionProfileCatalog, BuildingOwnership,
    BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog,
    LocalPosition, NavigationConfig, OccupancyCatalogs, SettlementKind, SettlementOwnership,
    UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, UnitWorkCapabilities, WeaponCatalog,
    WorkPermissionDomain, WorldData, WorldPosition, assign_building_settlement,
    assign_unit_settlement, create_settlement, create_unit_with_ownership, place_player_building,
    set_unit_work_permission, unit_may_autonomously_perform_work, unit_work_allowed,
    work_permission_domain_for_task,
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

fn settlement_with_worker(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
) -> (crate::world::SettlementId, crate::world::UnitId) {
    let settlement_id = create_settlement(
        world,
        pos(64.0, 64.0),
        "Test Town",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let worker = create_unit_with_ownership(
        unit_catalog,
        world,
        &UnitDefinitionId::new("bandit"),
        pos(60.0, 64.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    assign_unit_settlement(world, worker, Some(settlement_id)).unwrap();
    (settlement_id, worker)
}

fn run_assign(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapons: &WeaponCatalog,
    doodad: &DoodadCatalog,
    building: &crate::world::BuildingCatalog,
    tick: u64,
) -> crate::world::WorkerAssignmentReport {
    static INVENTORY_CTX: std::sync::OnceLock<(
        crate::world::ItemCatalog,
        crate::world::ItemCategoryCatalog,
        crate::world::InventoryProfileCatalog,
    )> = std::sync::OnceLock::new();
    let (items, categories, profiles) = INVENTORY_CTX.get_or_init(|| {
        (
            crate::world::ItemCatalog::default(),
            crate::world::ItemCategoryCatalog::default(),
            crate::world::InventoryProfileCatalog::default(),
        )
    });
    let inventory_ctx =
        crate::world::inventory::InventoryCatalogCtx::new(items, categories, profiles);
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    let operation_catalog = OperationCatalog::default();
    let mut ctx = WorkerAssignmentContext {
        world,
        unit_catalog,
        weapon_catalog: weapons,
        doodad_catalog: doodad,
        building_catalog: building,
        operation_catalog: &operation_catalog,
        interaction_catalog: &interaction,
        nav_config: &nav,
        inventory_ctx: &inventory_ctx,
        simulation_tick: tick,
    };
    step_worker_assignment(&mut ctx)
}

#[test]
fn default_worker_allowed_for_all_domains() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let (settlement_id, worker) = settlement_with_worker(&mut world, &unit_catalog);
    for domain in WorkPermissionDomain::ALL {
        assert!(unit_work_allowed(&world, settlement_id, worker, domain));
    }
}

#[test]
fn set_permission_validates_settlement_membership() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let settlement_id = create_settlement(
        &mut world,
        pos(64.0, 64.0),
        "Town",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let outsider = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(10.0, 10.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let err = set_unit_work_permission(
        &mut world,
        settlement_id,
        outsider,
        WorkPermissionDomain::Farming,
        false,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::world::WorkforcePermissionError::UnitNotSettlementMember { .. }
    ));
}

#[test]
fn construction_disallowed_blocks_autonomous_construction_only() {
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let building = crate::world::BuildingCatalog::default();
    let weapons = WeaponCatalog::default();
    let unit_catalog = UnitCatalog::default();
    let occ = occ(&building, &doodad, &footprint);
    let mut world = flat_world();
    let (settlement_id, worker) = settlement_with_worker(&mut world, &unit_catalog);
    let building_id = place_player_building(
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
    assign_building_settlement(&mut world, building_id, Some(settlement_id)).unwrap();
    sync_construction_tasks(&mut world, &building, 1);
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Construction,
        false,
    )
    .unwrap();
    let report = run_assign(&mut world, &unit_catalog, &weapons, &doodad, &building, 2);
    assert!(
        report.assignments.is_empty(),
        "construction denied should block autonomous assign: {:?}",
        report.diagnostics
    );
    let interaction = BuildingInteractionProfileCatalog::default();
    let nav = NavigationConfig::default();
    assert!(
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
            3,
        )
        .is_ok(),
        "player-assigned construction should bypass workforce permission"
    );
}

#[test]
fn general_labor_disallowed_does_not_block_farming_domain() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let (settlement_id, worker) = settlement_with_worker(&mut world, &unit_catalog);
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::GeneralLabor,
        false,
    )
    .unwrap();
    assert!(!unit_work_allowed(
        &world,
        settlement_id,
        worker,
        WorkPermissionDomain::GeneralLabor
    ));
    assert!(unit_work_allowed(
        &world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming
    ));
}

fn place_hut(
    world: &mut WorldData,
    building_catalog: &crate::world::BuildingCatalog,
) -> crate::world::BuildingId {
    place_production_building(
        world,
        building_catalog,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
    )
}

fn enable_operate_policy(
    world: &mut WorldData,
    building_id: crate::world::BuildingId,
    definition: &crate::world::building::catalog::BuildingDefinition,
    ops: &OperationCatalog,
    operation_id: crate::world::OperationDefinitionId,
) {
    let store = world.building_production_store_mut();
    store.ensure_policy_for_building(building_id, definition, ops);
    let policy = store.get_policy_mut(building_id);
    policy.enabled = true;
    policy.selected_operation = Some(operation_id);
}

fn place_production_building(
    world: &mut WorldData,
    building_catalog: &crate::world::BuildingCatalog,
    definition_id: &BuildingDefinitionId,
    at: WorldPosition,
) -> crate::world::BuildingId {
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occ = occ(building_catalog, &doodad, &footprint);
    place_player_building(
        &building_catalog,
        world,
        definition_id,
        at,
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id
}

#[test]
fn domain_mapping_uses_operation_category_not_building_name() {
    let mut world = flat_world();
    let building_catalog = crate::world::BuildingCatalog::default();
    let ops = OperationCatalog::default();
    let hut = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .expect("hut");
    let building_id = place_hut(&mut world, &building_catalog);
    enable_operate_policy(
        &mut world,
        building_id,
        hut,
        &ops,
        crate::world::OperationDefinitionId::new("mine_stone"),
    );
    let domain = work_permission_domain_for_task(
        &world,
        &building_catalog,
        &ops,
        building_id,
        TaskType::OperateWorkstation,
    );
    assert_eq!(domain, Some(WorkPermissionDomain::GeneralLabor));
}

#[test]
fn smelter_operate_workstation_is_unclassified_and_allowed() {
    let mut world = flat_world();
    let building_catalog = crate::world::BuildingCatalog::default();
    let ops = OperationCatalog::default();
    let hut = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .expect("hut");
    let building_id = place_hut(&mut world, &building_catalog);
    enable_operate_policy(
        &mut world,
        building_id,
        hut,
        &ops,
        crate::world::OperationDefinitionId::new("smelt_iron"),
    );
    assert!(
        work_permission_domain_for_task(
            &world,
            &building_catalog,
            &ops,
            building_id,
            TaskType::OperateWorkstation,
        )
        .is_none()
    );
    let unit_catalog = UnitCatalog::default();
    let (settlement_id, worker) = settlement_with_worker(&mut world, &unit_catalog);
    assign_building_settlement(&mut world, building_id, Some(settlement_id)).unwrap();
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming,
        false,
    )
    .unwrap();
    assert!(unit_may_autonomously_perform_work(
        &world,
        &building_catalog,
        &ops,
        worker,
        building_id,
        TaskType::OperateWorkstation,
    ));
}

#[test]
fn capability_does_not_override_disallowed_permission() {
    let mut world = flat_world();
    let caps = UnitWorkCapabilities::builder(2.0);
    let definition = crate::world::UnitDefinition::new(
        UnitDefinitionId::new("fast_builder"),
        "Fast Builder",
        crate::world::FactionId::new("player"),
        crate::world::SpeciesId::new("human"),
        "Player",
        5,
        10,
        10,
        10,
        10,
        10,
        10,
        10,
        10,
        50.0,
        "Elite",
        4.0,
        0.5,
        35.0,
        crate::world::WeaponDefinitionId::new("weapon_fists"),
        true,
        crate::world::UnitRenderKey::reserved("bandit"),
    )
    .with_work_capabilities(caps);
    let unit_catalog = UnitCatalog::from_definitions(vec![definition]).unwrap();
    let settlement_id = create_settlement(
        &mut world,
        pos(64.0, 64.0),
        "Test Town",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("fast_builder"),
        pos(60.0, 64.0),
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
    let building_catalog = crate::world::BuildingCatalog::default();
    let ops = OperationCatalog::default();
    let hut = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .expect("hut");
    let record = crate::world::create_building(
        &building_catalog,
        &mut world,
        &hut.id,
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
    )
    .unwrap();
    let building_id = record.id;
    assign_building_settlement(&mut world, building_id, Some(settlement_id)).unwrap();
    assert!(!unit_may_autonomously_perform_work(
        &world,
        &building_catalog,
        &ops,
        worker,
        building_id,
        TaskType::ConstructBuilding,
    ));
}

#[test]
fn reassigned_unit_does_not_inherit_previous_settlement_denials() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let settlement_a = create_settlement(
        &mut world,
        pos(10.0, 10.0),
        "A",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        Some(48.0),
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let settlement_b = create_settlement(
        &mut world,
        pos(200.0, 200.0),
        "B",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        Some(48.0),
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let worker = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(41.0, 41.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    assign_unit_settlement(&mut world, worker, Some(settlement_a)).unwrap();
    set_unit_work_permission(
        &mut world,
        settlement_a,
        worker,
        WorkPermissionDomain::GeneralLabor,
        false,
    )
    .unwrap();
    assign_unit_settlement(&mut world, worker, Some(settlement_b)).unwrap();
    assert!(unit_work_allowed(
        &world,
        settlement_b,
        worker,
        WorkPermissionDomain::GeneralLabor
    ));
}

#[test]
fn prispod_farm_definition_maps_to_farming_domain() {
    let building_catalog = crate::world::BuildingCatalog::default();
    let farm = building_catalog
        .get(&BuildingDefinitionId::new("prispod_farm"))
        .expect("prispod_farm");
    assert!(is_prispod_farm_definition(farm));
}

#[test]
fn farming_permission_denied_blocks_autonomous_operate_gate() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let (settlement_id, worker) = settlement_with_worker(&mut world, &unit_catalog);
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming,
        false,
    )
    .unwrap();
    assert!(!unit_work_allowed(
        &world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming
    ));
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming,
        true,
    )
    .unwrap();
    assert!(unit_work_allowed(
        &world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming
    ));
}

#[test]
fn permission_domain_taxonomy_has_six_current_categories() {
    assert_eq!(WorkPermissionDomain::ALL.len(), 6);
    assert!(WorkPermissionDomain::ALL.contains(&WorkPermissionDomain::Farming));
    assert!(WorkPermissionDomain::ALL.contains(&WorkPermissionDomain::GeneralLabor));
    assert!(WorkPermissionDomain::ALL.contains(&WorkPermissionDomain::Construction));
    assert!(WorkPermissionDomain::ALL.contains(&WorkPermissionDomain::Cooking));
    assert!(WorkPermissionDomain::ALL.contains(&WorkPermissionDomain::Science));
    assert!(WorkPermissionDomain::ALL.contains(&WorkPermissionDomain::Smithing));
}

#[test]
fn general_labor_denied_blocks_haul_and_extraction() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let building_catalog = crate::world::BuildingCatalog::default();
    let ops = OperationCatalog::default();
    let (settlement_id, worker) = settlement_with_worker(&mut world, &unit_catalog);
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::GeneralLabor,
        false,
    )
    .unwrap();
    let hut = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .expect("hut");
    let quarry_id = place_hut(&mut world, &building_catalog);
    enable_operate_policy(
        &mut world,
        quarry_id,
        hut,
        &ops,
        crate::world::OperationDefinitionId::new("mine_stone"),
    );
    assign_building_settlement(&mut world, quarry_id, Some(settlement_id)).unwrap();
    assert!(!unit_may_autonomously_perform_work(
        &world,
        &building_catalog,
        &ops,
        worker,
        quarry_id,
        TaskType::OperateWorkstation,
    ));
    assert!(!unit_may_autonomously_perform_work(
        &world,
        &building_catalog,
        &ops,
        worker,
        quarry_id,
        TaskType::Haul,
    ));
}

#[test]
fn cooking_permission_does_not_block_farming_or_construction() {
    let mut world = flat_world();
    let unit_catalog = UnitCatalog::default();
    let building_catalog = crate::world::BuildingCatalog::default();
    let ops = OperationCatalog::default();
    let (settlement_id, worker) = settlement_with_worker(&mut world, &unit_catalog);
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Cooking,
        false,
    )
    .unwrap();
    let hut_id = place_hut(&mut world, &building_catalog);
    assign_building_settlement(&mut world, hut_id, Some(settlement_id)).unwrap();
    assert!(unit_may_autonomously_perform_work(
        &world,
        &building_catalog,
        &ops,
        worker,
        hut_id,
        TaskType::ConstructBuilding,
    ));
}
